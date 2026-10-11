// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/private/SkGainmapInfo.h, src/codec/SkGainmapInfo.cpp

//! Gainmap rendering parameters (`SkGainmapInfo`) and their ISO 21496-1 serialization.
//!
//! Suppose a display has an HDR to SDR ratio of `H`. The gainmap math is documented on
//! [`GainmapInfo`]. The parameters are read from and written to the ISO 21496-1 blob, and the
//! Adobe `hdrgm` XMP parameters are converted into them by the codec crate.

use crate::color::Color4f;
use crate::color_space::ColorSpace;
use crate::data::Data;
use crate::stream::{DynamicMemoryWStream, MemoryStream, Stream, WStream};
use crate::stream_priv::{
    read_s32_be, read_u16_be, read_u32_be, write_s32_be, write_u16_be, write_u32_be,
};

const K_IS_MULTI_CHANNEL_MASK: u8 = 1 << 7;
const K_USE_BASE_COLOUR_SPACE_MASK: u8 = 1 << 6;

/// Gainmap rendering parameters (`SkGainmapInfo`).
///
/// Suppose our display has HDR to SDR ratio of H and we wish to display an image with gainmap on
/// this display. Let B be the pixel value from the base image in a color space that has the
/// primaries of the base image and a linear transfer function. Let G be the pixel value from the
/// gainmap. Let D be the output pixel in the same color space as B. The value of D is computed as
/// follows:
///
/// First, let W be a weight parameter determing how much the gainmap will be applied.
/// `W = clamp((log(H) - log(display_ratio_sdr)) / (log(display_ratio_hdr) - log(display_ratio_sdr), 0, 1)`
///
/// Next, let L be the gainmap value in log space. We compute this from the value G that was
/// sampled from the texture as follows:
/// `L = mix(log(gainmap_ratio_min), log(gainmap_ratio_max), pow(G, gainmap_gamma))`
///
/// Finally, apply the gainmap to compute D, the displayed pixel. If the base image is SDR then
/// compute: `D = (B + epsilon_sdr) * exp(L * W) - epsilon_hdr`. If the base image is HDR then
/// compute: `D = (B + epsilon_hdr) * exp(L * (W - 1)) - epsilon_sdr`.
///
/// This product includes Gain Map technology under license by Adobe.
#[doc(alias = "SkGainmapInfo")]
#[derive(Clone, Debug, PartialEq)]
pub struct GainmapInfo {
    /// Parameters for converting the gainmap from its image encoding to log space. These are
    /// specified per color channel. The alpha value is unused.
    pub gainmap_ratio_min: Color4f,
    pub gainmap_ratio_max: Color4f,
    pub gainmap_gamma: Color4f,

    /// Parameters sometimes used in gainmap computation to avoid numerical instability.
    pub epsilon_sdr: Color4f,
    pub epsilon_hdr: Color4f,

    /// If the output display's HDR to SDR ratio is less or equal than `display_ratio_sdr` then the
    /// SDR rendition is displayed. If it is greater or equal than `display_ratio_hdr` then the HDR
    /// rendition is displayed. Between these values an interpolation is displayed.
    pub display_ratio_sdr: f32,
    pub display_ratio_hdr: f32,

    /// Whether the base image is the SDR image or the HDR image.
    pub base_image_type: BaseImageType,

    /// The type of the gainmap image. See [`GainmapType`].
    pub gainmap_type: GainmapType,

    /// If specified, color space to apply the gainmap in, otherwise the base image's color space
    /// is used. Only the color primaries are used, the transfer function is irrelevant.
    pub gainmap_math_color_space: Option<ColorSpace>,
}

/// Whether the base image is the SDR image or the HDR image (`SkGainmapInfo::BaseImageType`).
#[doc(alias = "SkGainmapInfo::BaseImageType")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BaseImageType {
    #[default]
    Sdr,
    Hdr,
}

/// The type of the gainmap image (`SkGainmapInfo::Type`). If the type is `Apple`, the gainmap
/// image was originally encoded according to Apple's specification, and can be converted to the
/// `Default` type by applying the transformation described at Google's document.
#[doc(alias = "SkGainmapInfo::Type")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GainmapType {
    #[default]
    Default,
    Apple,
}

// Port of: include/private/SkGainmapInfo.h#L31-L62 (chrome/m156), the in-class initializers.
impl Default for GainmapInfo {
    fn default() -> Self {
        Self {
            gainmap_ratio_min: Color4f {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            gainmap_ratio_max: Color4f {
                r: 2.0,
                g: 2.0,
                b: 2.0,
                a: 1.0,
            },
            gainmap_gamma: Color4f {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            epsilon_sdr: Color4f {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            epsilon_hdr: Color4f {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            display_ratio_sdr: 1.0,
            display_ratio_hdr: 2.0,
            base_image_type: BaseImageType::Sdr,
            gainmap_type: GainmapType::Default,
            gainmap_math_color_space: None,
        }
    }
}

// Port of: src/codec/SkGainmapInfo.cpp#L32-L51 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // static_cast<int32_t> of llround's result
fn write_rational_be(s: &mut dyn WStream, x: f32) {
    // TODO(b/338342146): Select denominator to get maximum precision and robustness.
    let mut denominator: u32 = 0x1000_0000;
    if x.abs() > 1.0 {
        denominator = 0x1000;
    }
    // `llround` rounds half away from zero, which is `f64::round`.
    let numerator = (f64::from(x) * f64::from(denominator)).round() as i64 as i32;
    write_s32_be(s, numerator);
    write_u32_be(s, denominator);
}

// Port of: src/codec/SkGainmapInfo.cpp#L53-L64 (chrome/m156)
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // C++ casts of llround's result
fn write_positive_rational_be(s: &mut dyn WStream, x: f32) {
    // TODO(b/338342146): Select denominator to get maximum precision and robustness.
    let mut denominator: u32 = 0x1000_0000;
    if x > 1.0 {
        denominator = 0x1000;
    }
    let numerator = (f64::from(x) * f64::from(denominator)).round() as i64 as u32;
    write_u32_be(s, numerator);
    write_u32_be(s, denominator);
}

// Port of: src/codec/SkGainmapInfo.cpp#L66-L77 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // static_cast<float> of a double quotient
fn read_rational_be(s: &mut dyn Stream) -> Option<f32> {
    let numerator = read_s32_be(s)?;
    let denominator = read_u32_be(s)?;
    Some((f64::from(numerator) / f64::from(denominator)) as f32)
}

// Port of: src/codec/SkGainmapInfo.cpp#L79-L90 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // static_cast<float> of a double quotient
fn read_positive_rational_be(s: &mut dyn Stream) -> Option<f32> {
    let numerator = read_u32_be(s)?;
    let denominator = read_u32_be(s)?;
    Some((f64::from(numerator) / f64::from(denominator)) as f32)
}

// Port of: src/codec/SkGainmapInfo.cpp#L92-L110 (chrome/m156)
fn read_iso_gainmap_version(s: &mut dyn Stream) -> bool {
    // Ensure minimum version is 0.
    let Some(minimum_version) = read_u16_be(s) else {
        // SkCodecPrintf("Failed to read ISO 21496-1 minimum version.\n");
        return false;
    };
    if minimum_version != 0 {
        // SkCodecPrintf("Unsupported ISO 21496-1 minimum version.\n");
        return false;
    }

    // Ensure writer version is present. No value is invalid.
    if read_u16_be(s).is_none() {
        // SkCodecPrintf("Failed to read ISO 21496-1 version.\n");
        return false;
    }
    true
}

// Port of: src/codec/SkGainmapInfo.cpp#L112-L178 (chrome/m156)
fn read_iso_gainmap_info(s: &mut dyn Stream, info: &mut GainmapInfo) -> bool {
    if !read_iso_gainmap_version(s) {
        return false;
    }

    let Some(flags) = s.read_u8() else {
        return false;
    };
    let is_multi_channel = (flags & K_IS_MULTI_CHANNEL_MASK) != 0;
    let use_base_colour_space = (flags & K_USE_BASE_COLOUR_SPACE_MASK) != 0;

    let Some(base_hdr_headroom) = read_positive_rational_be(s) else {
        return false;
    };
    let Some(altr_hdr_headroom) = read_positive_rational_be(s) else {
        return false;
    };

    let mut gain_map_min = [0.0f32; 3];
    let mut gain_map_max = [0.0f32; 3];
    let mut gamma = [0.0f32; 3];
    let mut base_offset = [0.0f32; 3];
    let mut altr_offset = [0.0f32; 3];

    let channel_count = if is_multi_channel { 3 } else { 1 };
    for i in 0..channel_count {
        let Some(v) = read_rational_be(s) else {
            return false;
        };
        gain_map_min[i] = v;
        let Some(v) = read_rational_be(s) else {
            return false;
        };
        gain_map_max[i] = v;
        let Some(v) = read_positive_rational_be(s) else {
            return false;
        };
        gamma[i] = v;
        let Some(v) = read_rational_be(s) else {
            return false;
        };
        base_offset[i] = v;
        let Some(v) = read_rational_be(s) else {
            return false;
        };
        altr_offset[i] = v;
    }

    *info = GainmapInfo::default();
    if !use_base_colour_space {
        info.gainmap_math_color_space = Some(ColorSpace::new_srgb());
    }
    if base_hdr_headroom < altr_hdr_headroom {
        info.base_image_type = BaseImageType::Sdr;
        info.display_ratio_sdr = base_hdr_headroom.exp2();
        info.display_ratio_hdr = altr_hdr_headroom.exp2();
    } else {
        info.base_image_type = BaseImageType::Hdr;
        info.display_ratio_hdr = base_hdr_headroom.exp2();
        info.display_ratio_sdr = altr_hdr_headroom.exp2();
    }
    for i in 0..3 {
        let j = if i >= channel_count { 0 } else { i };
        *channel_mut(&mut info.gainmap_ratio_min, i) = gain_map_min[j].exp2();
        *channel_mut(&mut info.gainmap_ratio_max, i) = gain_map_max[j].exp2();
        *channel_mut(&mut info.gainmap_gamma, i) = 1.0 / gamma[j];
        match info.base_image_type {
            BaseImageType::Sdr => {
                *channel_mut(&mut info.epsilon_sdr, i) = base_offset[j];
                *channel_mut(&mut info.epsilon_hdr, i) = altr_offset[j];
            }
            BaseImageType::Hdr => {
                *channel_mut(&mut info.epsilon_hdr, i) = base_offset[j];
                *channel_mut(&mut info.epsilon_sdr, i) = altr_offset[j];
            }
        }
    }
    true
}

// Indexes the RGB channels in the order `SkColor4f::operator[]` does, for the per-channel loops.
fn channel(c: &Color4f, i: usize) -> f32 {
    match i {
        0 => c.r,
        1 => c.g,
        _ => c.b,
    }
}

fn channel_mut(c: &mut Color4f, i: usize) -> &mut f32 {
    match i {
        0 => &mut c.r,
        1 => &mut c.g,
        _ => &mut c.b,
    }
}

impl GainmapInfo {
    /// Return true if this can be encoded as an `UltraHDR` v1 image.
    // Port of: src/codec/SkGainmapInfo.cpp#L180-L192 (chrome/m156)
    #[must_use]
    pub fn is_ultra_hdr_v1_compatible(&self) -> bool {
        // UltraHDR v1 supports having the base image be HDR in theory, but it is largely
        // untested.
        if self.base_image_type == BaseImageType::Hdr {
            return false;
        }
        // UltraHDR v1 doesn't support a non-base gainmap math color space.
        if self.gainmap_math_color_space.is_some() {
            return false;
        }
        true
    }

    /// If `data` contains an ISO 21496-1 version that is supported, return true. Otherwise return
    /// false.
    // Port of: src/codec/SkGainmapInfo.cpp#L194-L200 (chrome/m156)
    #[must_use]
    pub fn parse_version(data: Option<&Data>) -> bool {
        let Some(data) = data else {
            return false;
        };
        let mut s = MemoryStream::from_data(Some(data.clone()));
        read_iso_gainmap_version(&mut s)
    }

    /// If `data` constains ISO 21496-1 metadata then parse that metadata then use it to populate
    /// `info` and return true, otherwise return false. If `data` indicates that the base image
    /// color space primaries should be used for gainmap application then
    /// `gainmap_math_color_space` is set to `None`, otherwise it is set to sRGB (the default, to be
    /// overwritten by the image decoder).
    // Port of: src/codec/SkGainmapInfo.cpp#L202-L208 (chrome/m156)
    pub fn parse(data: Option<&Data>, info: &mut GainmapInfo) -> bool {
        let Some(data) = data else {
            return false;
        };
        let mut s = MemoryStream::from_data(Some(data.clone()));
        read_iso_gainmap_info(&mut s, info)
    }

    /// Serialize an ISO 21496-1 version 0 blob containing only the version structure.
    // Port of: src/codec/SkGainmapInfo.cpp#L210-L216 (chrome/m156)
    #[must_use]
    pub fn serialize_version() -> Data {
        let mut s = DynamicMemoryWStream::new();
        write_u16_be(&mut s, 0); // Minimum reader version
        write_u16_be(&mut s, 0); // Writer version
        s.detach_as_data()
    }

    /// Serialize an ISO 21496-1 version 0 blob containing this gainmap's parameters.
    // Port of: src/codec/SkGainmapInfo.cpp#L220-L271 (chrome/m156)
    #[must_use]
    pub fn serialize(&self) -> Data {
        let mut s = DynamicMemoryWStream::new();
        // Version.
        write_u16_be(&mut s, 0); // Minimum reader version
        write_u16_be(&mut s, 0); // Writer version

        // Flags.
        let all_single_channel = is_single_channel(self.gainmap_ratio_min)
            && is_single_channel(self.gainmap_ratio_max)
            && is_single_channel(self.gainmap_gamma)
            && is_single_channel(self.epsilon_sdr)
            && is_single_channel(self.epsilon_hdr);
        let mut flags: u8 = 0;
        if self.gainmap_math_color_space.is_none() {
            flags |= K_USE_BASE_COLOUR_SPACE_MASK;
        }
        if !all_single_channel {
            flags |= K_IS_MULTI_CHANNEL_MASK;
        }
        s.write8(flags);

        // Base and altr headroom.
        match self.base_image_type {
            BaseImageType::Sdr => {
                write_positive_rational_be(&mut s, self.display_ratio_sdr.log2());
                write_positive_rational_be(&mut s, self.display_ratio_hdr.log2());
            }
            BaseImageType::Hdr => {
                write_positive_rational_be(&mut s, self.display_ratio_hdr.log2());
                write_positive_rational_be(&mut s, self.display_ratio_sdr.log2());
            }
        }

        // Per-channel information.
        let channels = if all_single_channel { 1 } else { 3 };
        for i in 0..channels {
            write_rational_be(&mut s, channel(&self.gainmap_ratio_min, i).log2());
            write_rational_be(&mut s, channel(&self.gainmap_ratio_max, i).log2());
            write_positive_rational_be(&mut s, 1.0 / channel(&self.gainmap_gamma, i));
            match self.base_image_type {
                BaseImageType::Sdr => {
                    write_rational_be(&mut s, channel(&self.epsilon_sdr, i));
                    write_rational_be(&mut s, channel(&self.epsilon_hdr, i));
                }
                BaseImageType::Hdr => {
                    write_rational_be(&mut s, channel(&self.epsilon_hdr, i));
                    write_rational_be(&mut s, channel(&self.epsilon_sdr, i));
                }
            }
        }
        s.detach_as_data()
    }
}

// Port of: src/codec/SkGainmapInfo.cpp#L218-L225 (chrome/m156)
#[allow(clippy::float_cmp)] // exact comparison, as in C++
fn is_single_channel(c: Color4f) -> bool {
    c.r == c.g && c.g == c.b
}

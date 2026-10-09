// Copyright (C) 1998-2025 Glenn Randers-Pehrson and the libpng contributors.
// Use of this source code is governed by the libpng licence (libpng-2.0) in the LICENSE file.
// Port of: pngget.c (libpng 1.6.56, skia.googlesource.com/third_party/libpng@d5515b5b), the
// accessors the codec uses.

// Clippy: each module is a line-by-line port of libpng's C, whose integer casts, long
// functions, argument lists and error returns are kept as written so they can be compared
// with the C. The Port of links name the C source for each item.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::missing_errors_doc,
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::cognitive_complexity
)]
use crate::structs::{PngColor, PngColor8, PngColor16, PngInfo, PngStruct, info};

/// Port of `png_get_valid` (pngget.c#L2-L10): whether a `PNG_INFO_*` chunk was read.
#[doc(alias = "png_get_valid")]
#[must_use]
pub fn get_valid(info: &PngInfo, flag: u32) -> bool {
    info.valid & flag != 0
}

/// Port of `png_get_rowbytes` (pngget.c#L14-L19).
#[doc(alias = "png_get_rowbytes")]
#[must_use]
pub fn get_rowbytes(info: &PngInfo) -> usize {
    info.rowbytes
}

/// Port of `png_get_IHDR` (pngget.c#L634-L660): width, height, bit depth, colour type,
/// interlace, compression and filter.
#[doc(alias = "png_get_IHDR")]
#[must_use]
pub fn get_ihdr(info: &PngInfo) -> (u32, u32, u8, u8, u8, u8, u8) {
    (
        info.width,
        info.height,
        info.bit_depth,
        info.color_type,
        info.interlace_type,
        info.compression_type,
        info.filter_type,
    )
}

/// Port of `png_get_PLTE` (pngget.c#L773-L785). `None` when no palette was read.
#[doc(alias = "png_get_PLTE")]
#[must_use]
pub fn get_plte(info: &PngInfo) -> Option<&[PngColor]> {
    if info.valid & info::PLTE != 0 {
        Some(&info.palette)
    } else {
        None
    }
}

/// Port of `png_get_tRNS` (pngget.c#L832-L866): the palette alpha values (or `None`) and the
/// colour key (or `None`).
#[doc(alias = "png_get_tRNS")]
#[must_use]
pub fn get_trns(info: &PngInfo) -> Option<(&[u8], Option<PngColor16>)> {
    if info.valid & info::TRNS == 0 {
        return None;
    }
    let alpha = &info.trans_alpha[..info.num_trans as usize];
    let color = if info.color_type == 0 || info.color_type == 2 {
        Some(info.trans_color)
    } else {
        None
    };
    Some((alpha, color))
}

/// Port of `png_get_iCCP` (pngget.c#L466-L482): the profile name, compression and profile bytes.
#[doc(alias = "png_get_iCCP")]
#[must_use]
pub fn get_iccp(info: &PngInfo) -> Option<(&str, u8, &[u8])> {
    if info.valid & info::ICCP != 0 {
        Some((&info.iccp_name, info.iccp_compression, &info.iccp_profile))
    } else {
        None
    }
}

/// Port of `png_get_cHRM_fixed` (pngget.c#L403-L423): the eight fixed-point values.
#[doc(alias = "png_get_cHRM_fixed")]
#[must_use]
pub fn get_chrm_fixed(info: &PngInfo) -> Option<[i32; 8]> {
    if info.valid & info::CHRM == 0 {
        return None;
    }
    let mut out = [0i32; 8];
    for (o, v) in out.iter_mut().zip(info.int_chrm.iter()) {
        *o = *v as i32;
    }
    Some(out)
}

/// Port of `png_get_gAMA_fixed` (pngget.c#L425-L436).
#[doc(alias = "png_get_gAMA_fixed")]
#[must_use]
pub fn get_gama_fixed(info: &PngInfo) -> Option<i32> {
    if info.valid & info::GAMA != 0 {
        Some(info.gamma as i32)
    } else {
        None
    }
}

/// Port of `png_get_sBIT` (pngget.c#L788-L798).
#[doc(alias = "png_get_sBIT")]
#[must_use]
pub fn get_sbit(info: &PngInfo) -> Option<PngColor8> {
    if info.valid & info::SBIT != 0 {
        Some(info.sig_bit)
    } else {
        None
    }
}

/// Port of `png_get_sRGB` (pngget.c#L452-L464).
#[doc(alias = "png_get_sRGB")]
#[must_use]
pub fn get_srgb(info: &PngInfo) -> Option<u8> {
    if info.valid & info::SRGB != 0 {
        Some(info.srgb_intent)
    } else {
        None
    }
}

impl PngStruct {
    /// Port of `png_get_progressive_ptr`: the progressive handler is stored in the struct, so
    /// there is no separate user pointer. Returns whether a handler is installed.
    #[doc(alias = "png_get_progressive_ptr")]
    #[must_use]
    pub fn has_progressive_handler(&self) -> bool {
        self.progressive.is_some()
    }
}

// Port of: libjpeg-turbo src/jccolor.c#L1-L560 and src/jccolext.c#L1-L152 (libjpeg_turbo@e14cbfaa,
// 3.1.0 source): `rgb_ycc_start`, `rgb_ycc_convert` and its extended-layout variants, `rgb_gray_convert`,
// `grayscale_convert`, `null_convert`, and the checks of `_jinit_color_converter`. The rows are
// stored, unpadded, into the component planes that `prep.rs` and `sample.rs` then read.
//
// Copyright (C) 1991-1998, Thomas G. Lane. Modified 2009-2011 by D. R. Commander.
// Copyright (C) 2015, 2017, 2019, 2024, 2025 D. R. Commander (libjpeg-turbo extensions).
// Copyright (C) 2025 The skia-rust Authors.

use super::Compress;
use crate::error::{Error, Result};
use crate::tables::ColorSpace;

/// `SCALEBITS`: the fixed-point scale of the colour matrix.
const SCALEBITS: u32 = 16;
/// `CBCR_OFFSET`: `CENTERJSAMPLE << SCALEBITS`.
const CBCR_OFFSET: i32 = 128 << SCALEBITS;
/// `ONE_HALF`: the rounding term.
const ONE_HALF: i32 = 1 << (SCALEBITS - 1);
/// `R_Y_OFF`, `G_Y_OFF`, ... : the offsets of each product table in `rgb_ycc_tab`.
const R_Y_OFF: usize = 0;
const G_Y_OFF: usize = 256;
const B_Y_OFF: usize = 512;
const R_CB_OFF: usize = 768;
const G_CB_OFF: usize = 1024;
const B_CB_OFF: usize = 1280;
/// `R_CR_OFF` is `B_CB_OFF`: the Cr product of red shares the Cb table's rounding entry.
const R_CR_OFF: usize = B_CB_OFF;
const G_CR_OFF: usize = 1536;
const B_CR_OFF: usize = 1792;
/// `TABLE_SIZE`: `8 * (MAXJSAMPLE + 1)`.
const TABLE_SIZE: usize = 8 * 256;

/// `FIX(x)` (`jccolor.c`): the colour coefficient in 16-bit fixed point.
fn fix(x: f64) -> i32 {
    (x * 65536.0 + 0.5) as i32
}

/// `rgb_ycc_start`: the product tables `rgb_ycc_tab`, one entry per sample value and channel.
fn make_rgb_ycc_table() -> Vec<i32> {
    let mut tab = vec![0i32; TABLE_SIZE];
    for i in 0..=255i32 {
        let iu = i as usize;
        tab[iu + R_Y_OFF] = fix(0.29900) * i;
        tab[iu + G_Y_OFF] = fix(0.58700) * i;
        tab[iu + B_Y_OFF] = fix(0.11400) * i + ONE_HALF;
        tab[iu + R_CB_OFF] = (-fix(0.16874)) * i;
        tab[iu + G_CB_OFF] = (-fix(0.33126)) * i;
        tab[iu + B_CB_OFF] = fix(0.50000) * i + CBCR_OFFSET + ONE_HALF - 1;
        tab[iu + G_CR_OFF] = (-fix(0.41869)) * i;
        tab[iu + B_CR_OFF] = (-fix(0.08131)) * i;
    }
    tab
}

/// The byte offsets of red, green and blue within one pixel, and the pixel size, for each
/// RGB-type input colour space (`rgb_red`, `rgb_green`, `rgb_blue`, `rgb_pixelsize` in
/// `jmorecfg.h`). `None` for the others.
fn rgb_layout(cs: ColorSpace) -> Option<([usize; 3], usize)> {
    match cs {
        ColorSpace::Rgb | ColorSpace::ExtRgb => Some(([0, 1, 2], 3)),
        ColorSpace::ExtRgbx | ColorSpace::ExtRgba => Some(([0, 1, 2], 4)),
        ColorSpace::ExtBgr => Some(([2, 1, 0], 3)),
        ColorSpace::ExtBgrx | ColorSpace::ExtBgra => Some(([2, 1, 0], 4)),
        ColorSpace::ExtXbgr | ColorSpace::ExtAbgr => Some(([3, 2, 1], 4)),
        ColorSpace::ExtXrgb | ColorSpace::ExtArgb => Some(([1, 2, 3], 4)),
        _ => None,
    }
}

/// Which conversion `jinit_color_converter` selected (its `_color_convert` pointer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ColorConversion {
    /// `null_convert` with three components (YCbCr input to YCbCr output).
    NullYCbCr,
    /// `grayscale_convert`: a single channel copied.
    Grayscale,
    /// `rgb_ycc_convert`: RGB-type input to YCbCr output.
    RgbToYcc {
        offsets: [usize; 3],
        pixelsize: usize,
    },
    /// `rgb_gray_convert`: RGB-type input to grayscale output (luminance only).
    RgbToGray {
        offsets: [usize; 3],
        pixelsize: usize,
    },
}

impl Compress {
    /// The checks of `_jinit_color_converter` and the choice of conversion for
    /// `(in_color_space, jpeg_color_space)`.
    pub(super) fn color_conversion_kind(&self) -> Result<ColorConversion> {
        let in_cs = self.in_color_space;
        // The input component count must match the colour space.
        match in_cs {
            ColorSpace::Grayscale => {
                if self.input_components != 1 {
                    return Err(Error::BadColorspace);
                }
            }
            ColorSpace::YCbCr => {
                if self.input_components != 3 {
                    return Err(Error::BadColorspace);
                }
            }
            ColorSpace::Cmyk | ColorSpace::Ycck => {
                if self.input_components != 4 {
                    return Err(Error::BadColorspace);
                }
            }
            _ => {
                if let Some((_, pixelsize)) = rgb_layout(in_cs) {
                    if self.input_components != pixelsize as i32 {
                        return Err(Error::BadColorspace);
                    }
                } else if self.input_components < 1 {
                    return Err(Error::BadColorspace);
                }
            }
        }
        match self.jpeg_color_space {
            ColorSpace::Grayscale => {
                if self.num_components != 1 {
                    return Err(Error::BadColorspace);
                }
                if in_cs == ColorSpace::Grayscale {
                    Ok(ColorConversion::Grayscale)
                } else if let Some((offsets, pixelsize)) = rgb_layout(in_cs) {
                    Ok(ColorConversion::RgbToGray { offsets, pixelsize })
                } else {
                    Err(Error::ConversionNotImplemented)
                }
            }
            ColorSpace::YCbCr => {
                if self.num_components != 3 {
                    return Err(Error::BadColorspace);
                }
                if let Some((offsets, pixelsize)) = rgb_layout(in_cs) {
                    Ok(ColorConversion::RgbToYcc { offsets, pixelsize })
                } else if in_cs == ColorSpace::YCbCr {
                    Ok(ColorConversion::NullYCbCr)
                } else {
                    Err(Error::ConversionNotImplemented)
                }
            }
            _ => Err(Error::ConversionNotImplemented),
        }
    }

    /// `rgb_ycc_convert` and its siblings for one row: appends the converted samples of each
    /// component to its plane (`image_width` samples per row).
    pub(super) fn pre_process_row(&mut self, row: &[u8]) -> Result<()> {
        let kind = self.color_conversion_kind()?;
        let width = self.image_width as usize;
        let nc_in = self.input_components as usize;
        if row.len() < width * nc_in {
            return Err(Error::BufferSize);
        }
        if self.planes.len() != self.num_components {
            self.planes = vec![Vec::new(); self.num_components];
        }
        match kind {
            ColorConversion::NullYCbCr => {
                // null_convert, nc == 3: each channel is copied.
                for col in 0..width {
                    for ci in 0..3 {
                        self.planes[ci].push(row[col * 3 + ci]);
                    }
                }
            }
            ColorConversion::Grayscale => {
                // grayscale_convert: the single channel, with stride input_components.
                for col in 0..width {
                    self.planes[0].push(row[col * nc_in]);
                }
            }
            ColorConversion::RgbToYcc { offsets, pixelsize } => {
                if self.rgb_ycc_tab.is_empty() {
                    self.rgb_ycc_tab = make_rgb_ycc_table();
                }
                let ctab = &self.rgb_ycc_tab;
                for col in 0..width {
                    let at = col * pixelsize;
                    let red = row[at + offsets[0]] as usize;
                    let green = row[at + offsets[1]] as usize;
                    let blue = row[at + offsets[2]] as usize;
                    let y = (ctab[red + R_Y_OFF] + ctab[green + G_Y_OFF] + ctab[blue + B_Y_OFF])
                        >> SCALEBITS;
                    let cb =
                        (ctab[red + R_CB_OFF] + ctab[green + G_CB_OFF] + ctab[blue + B_CB_OFF])
                            >> SCALEBITS;
                    let cr =
                        (ctab[red + R_CR_OFF] + ctab[green + G_CR_OFF] + ctab[blue + B_CR_OFF])
                            >> SCALEBITS;
                    self.planes[0].push(y as u8);
                    self.planes[1].push(cb as u8);
                    self.planes[2].push(cr as u8);
                }
            }
            ColorConversion::RgbToGray { offsets, pixelsize } => {
                if self.rgb_ycc_tab.is_empty() {
                    self.rgb_ycc_tab = make_rgb_ycc_table();
                }
                let ctab = &self.rgb_ycc_tab;
                for col in 0..width {
                    let at = col * pixelsize;
                    let red = row[at + offsets[0]] as usize;
                    let green = row[at + offsets[1]] as usize;
                    let blue = row[at + offsets[2]] as usize;
                    let y = (ctab[red + R_Y_OFF] + ctab[green + G_Y_OFF] + ctab[blue + B_Y_OFF])
                        >> SCALEBITS;
                    self.planes[0].push(y as u8);
                }
            }
        }
        Ok(())
    }
}

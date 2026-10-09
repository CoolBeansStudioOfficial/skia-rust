// Port of: libjpeg-turbo src/jdcolor.c#L30-L300, #L309-L368, #L376-L587, #L775-L930 and
//          src/jdcolext.c#L29-L145 (libjpeg_turbo@e14cbfaa, 3.1.0)
//
// Copyright (C) 1991-1997, Thomas G. Lane. libjpeg-turbo Modifications: Copyright (C) 2011-2015,
// 2017, 2019, 2022, D. R. Commander. Rust port Copyright (C) 2025 The skia-rust Authors. Licence: IJG.
//
//! Colour conversion from the decoded component planes to the output: YCbCr to RGB (all the
//! extended RGB layouts), grayscale to RGB or gray, RGB to RGB layouts, YCCK to CMYK, and the
//! pass-through cases.
//!
//! RGB565 output (`jdcol565.c`) is ported for `JDITHER_NONE`, the only mode Skia uses. The
//! ordered-dither variants are reported as not implemented, see `jinit_color_deconverter`.
//!
//! Arithmetic: `FIX(x)` is `(JLONG)(x * 65536 + 0.5)`, `ONE_HALF` is `1 << 15`, and every shift is
//! the arithmetic shift of a 32-bit `JLONG`, as in the C code. `RIGHT_SHIFT` is `>>`.

// Clippy (pedantic) allows, for this module. Each one fires on the C arithmetic and naming this
// module mirrors, and the code is kept as the C writes it so it can be checked line by line:
// JLONG/int/JDIMENSION casts (sign, truncation and wrap), C operator precedence and identity
// terms that come out of macros (`x * 1`, `0 * n`), C loop shapes (`needless_range_loop`,
// `explicit_counter_loop`, `collapsible_if`, `match_same_arms`), the C variable names
// (`similar_names`, `struct_field_names`), libjpeg's constants written as in jdct.h
// (`approx_constant`, `unreadable_literal`), functions whose C form returns a status that
// this path never sets (`unnecessary_wraps`), and the long C routines (`too_many_lines`,
// `too_many_arguments`). Error docs point at the `Error` variants, which name the C codes.
#![allow(
    clippy::approx_constant,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::collapsible_if,
    clippy::doc_markdown,
    clippy::erasing_op,
    clippy::explicit_counter_loop,
    clippy::identity_op,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::needless_range_loop,
    clippy::precedence,
    clippy::similar_names,
    clippy::single_match_else,
    clippy::struct_field_names,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    clippy::unreadable_literal,
    clippy::unused_self
)]

use crate::Decompress;
use crate::decompress::DitherMode;
use crate::error::{Error, Result};
use crate::tables::ColorSpace;

/// `SCALEBITS`.
const SCALEBITS: i32 = 16;
/// `ONE_HALF`.
const ONE_HALF: i32 = 1 << (SCALEBITS - 1);

/// `FIX(x)` at `SCALEBITS` = 16.
#[inline]
const fn fix(x: f64) -> i32 {
    (x * 65536.0 + 0.5) as i32
}

/// The converter kinds (the `_color_convert` method of `my_color_deconverter`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ConvKind {
    /// Not selected (before `jinit_color_deconverter`).
    #[default]
    None,
    /// `ycc_rgb_convert` to an extended RGB layout.
    YccRgb,
    /// `gray_rgb_convert` to an extended RGB layout.
    GrayRgb,
    /// `rgb_rgb_convert` to an extended RGB layout (`rgb_red` etc. may differ from RGB).
    RgbRgb,
    /// `grayscale_convert`: copy the Y plane (JCS_GRAYSCALE output).
    Grayscale,
    /// `rgb_gray_convert`: RGB to Y (JCS_GRAYSCALE output from RGB data).
    RgbGray,
    /// `null_convert` for three components straight to JCS_RGB.
    NullRgb,
    /// `null_convert` for four components (CMYK).
    NullCmyk,
    /// `ycc_rgb565_convert` (JDITHER_NONE).
    YccRgb565,
    /// `gray_rgb565_convert` (JDITHER_NONE).
    GrayRgb565,
    /// `rgb_rgb565_convert` (JDITHER_NONE).
    RgbRgb565,
    /// `ycck_cmyk_convert`.
    YcckCmyk,
}

/// `PACK_SHORT_565_LE` (`jdcolor.c`): one RGB565 pixel from 8-bit components, as the
/// little-endian value of the `INT16` the C code stores.
#[inline]
fn pack_short_565(r: u32, g: u32, b: u32) -> u16 {
    (((r << 8) & 0xF800) | ((g << 3) & 0x7E0) | (b >> 3)) as u16
}

/// Stores one RGB565 pixel at `dst[0..2]` in host (little-endian) order.
#[inline]
fn store_565(dst: &mut [u8], v: u16) {
    dst[..2].copy_from_slice(&v.to_le_bytes());
}

/// `my_color_deconverter` (the tables and the selected method).
#[derive(Debug, Clone, Default)]
pub(crate) struct ColorState {
    /// Selected method.
    pub(crate) kind: ConvKind,
    /// `Cr_r_tab`.
    pub(crate) cr_r_tab: Vec<i32>,
    /// `Cb_b_tab`.
    pub(crate) cb_b_tab: Vec<i32>,
    /// `Cr_g_tab`.
    pub(crate) cr_g_tab: Vec<i32>,
    /// `Cb_g_tab`.
    pub(crate) cb_g_tab: Vec<i32>,
    /// `rgb_y_tab` (for the RGB-to-gray case).
    pub(crate) rgb_y_tab: Vec<i32>,
    /// `cconvert->_color_convert` is `noop_convert` (`read_and_discard_scanlines` in
    /// jdapistd.c): the rows are upsampled and counted, but not converted.
    pub(crate) discard: bool,
}

/// Byte offsets of the colour components in one output pixel, and the pixel size, for an
/// extended RGB layout (`EXT_*_RED/GREEN/BLUE/ALPHA` and `EXT_*_PIXELSIZE` of `jdcolext.c`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Layout {
    pub(crate) red: usize,
    pub(crate) green: usize,
    pub(crate) blue: usize,
    /// Offset of an alpha or padding byte that is set to 0xFF (`RGB_ALPHA`), if any.
    pub(crate) alpha: Option<usize>,
    pub(crate) pixelsize: usize,
}

/// The layout of an RGB-type output colour space.
pub(crate) fn layout(cs: ColorSpace) -> Option<Layout> {
    Some(match cs {
        ColorSpace::Rgb | ColorSpace::ExtRgb => Layout {
            red: 0,
            green: 1,
            blue: 2,
            alpha: None,
            pixelsize: 3,
        },
        ColorSpace::ExtRgbx | ColorSpace::ExtRgba => Layout {
            red: 0,
            green: 1,
            blue: 2,
            alpha: Some(3),
            pixelsize: 4,
        },
        ColorSpace::ExtBgr => Layout {
            red: 2,
            green: 1,
            blue: 0,
            alpha: None,
            pixelsize: 3,
        },
        ColorSpace::ExtBgrx | ColorSpace::ExtBgra => Layout {
            red: 2,
            green: 1,
            blue: 0,
            alpha: Some(3),
            pixelsize: 4,
        },
        ColorSpace::ExtXbgr | ColorSpace::ExtAbgr => Layout {
            red: 3,
            green: 2,
            blue: 1,
            alpha: Some(0),
            pixelsize: 4,
        },
        ColorSpace::ExtXrgb | ColorSpace::ExtArgb => Layout {
            red: 1,
            green: 2,
            blue: 3,
            alpha: Some(0),
            pixelsize: 4,
        },
        _ => return None,
    })
}

impl Decompress {
    /// `jinit_color_deconverter`: selects the conversion for the (JPEG colour space, output
    /// colour space) pair, as `jdcolor.c` does.
    pub(crate) fn jinit_color_deconverter(&mut self) -> Result<()> {
        if self.data_precision != 8 {
            return Err(Error::BadPrecision);
        }
        // Check the component count against the JPEG colour space.
        match self.jpeg_color_space {
            ColorSpace::Grayscale => {
                if self.num_components != 1 {
                    return Err(Error::BadColorspace);
                }
            }
            ColorSpace::Rgb | ColorSpace::YCbCr => {
                if self.num_components != 3 {
                    return Err(Error::BadColorspace);
                }
            }
            ColorSpace::Cmyk | ColorSpace::Ycck => {
                if self.num_components != 4 {
                    return Err(Error::BadColorspace);
                }
            }
            _ => {
                if self.num_components < 1 {
                    return Err(Error::BadColorspace);
                }
            }
        }
        let mut st = ColorState::default();
        match self.out_color_space {
            ColorSpace::Grayscale => {
                self.out_color_components = 1;
                match self.jpeg_color_space {
                    ColorSpace::Grayscale | ColorSpace::YCbCr => {
                        st.kind = ConvKind::Grayscale;
                        // Only the Y plane is needed.
                        for ci in 1..self.num_components as usize {
                            self.comp_info[ci].component_needed = false;
                        }
                    }
                    ColorSpace::Rgb => {
                        st.kind = ConvKind::RgbGray;
                        build_rgb_y_table(&mut st);
                    }
                    _ => return Err(Error::BadColorspace),
                }
            }
            ColorSpace::Rgb
            | ColorSpace::ExtRgb
            | ColorSpace::ExtRgbx
            | ColorSpace::ExtBgr
            | ColorSpace::ExtBgrx
            | ColorSpace::ExtXbgr
            | ColorSpace::ExtXrgb
            | ColorSpace::ExtRgba
            | ColorSpace::ExtBgra
            | ColorSpace::ExtAbgr
            | ColorSpace::ExtArgb => {
                self.out_color_components = crate::tables::rgb_pixelsize(self.out_color_space);
                match self.jpeg_color_space {
                    ColorSpace::YCbCr => {
                        st.kind = ConvKind::YccRgb;
                        build_ycc_rgb_table(&mut st);
                    }
                    ColorSpace::Grayscale => st.kind = ConvKind::GrayRgb,
                    ColorSpace::Rgb => {
                        // null_convert when the output is the plain RGB layout.
                        st.kind = if self.out_color_space == ColorSpace::Rgb {
                            ConvKind::NullRgb
                        } else {
                            ConvKind::RgbRgb
                        };
                    }
                    _ => return Err(Error::BadColorspace),
                }
            }
            ColorSpace::Rgb565 => {
                // jdcolor.c `JCS_RGB565`. Only JDITHER_NONE is ported (Skia sets it, see
                // SkJpegCodec.cpp#L320-L330). The ordered dither variants depend on the output
                // pointer's alignment (the first pixel of an unaligned row is not rotated), so
                // they are reported as not implemented rather than guessed.
                self.out_color_components = 3;
                if self.dither_mode != DitherMode::None {
                    return Err(Error::NotImplemented);
                }
                match self.jpeg_color_space {
                    ColorSpace::YCbCr => {
                        st.kind = ConvKind::YccRgb565;
                        build_ycc_rgb_table(&mut st);
                    }
                    ColorSpace::Grayscale => st.kind = ConvKind::GrayRgb565,
                    ColorSpace::Rgb => st.kind = ConvKind::RgbRgb565,
                    _ => return Err(Error::BadColorspace),
                }
            }
            ColorSpace::Cmyk => {
                self.out_color_components = 4;
                match self.jpeg_color_space {
                    ColorSpace::Ycck => {
                        st.kind = ConvKind::YcckCmyk;
                        build_ycc_rgb_table(&mut st);
                    }
                    ColorSpace::Cmyk => st.kind = ConvKind::NullCmyk,
                    _ => return Err(Error::BadColorspace),
                }
            }
            _ => {
                // default: JCS_RGB-like pass-through for the number of components.
                self.out_color_components = self.num_components;
                st.kind = ConvKind::NullRgb;
            }
        }
        self.cconvert = st;
        Ok(())
    }

    /// `start_pass_dcolor`: nothing per pass.
    pub(crate) fn start_pass_color_convert(&mut self) -> Result<()> {
        Ok(())
    }

    /// `_color_convert` for the selected method: converts `num_rows` rows starting at
    /// `input_row` of `self.upsample.color_buf` into `output`.
    pub(crate) fn color_convert(
        &self,
        input_row: usize,
        output: &mut [&mut [u8]],
        num_rows: usize,
    ) -> Result<()> {
        let buf = &self.upsample.color_buf;
        let rl = &*self.range_limit;
        let st = &self.cconvert;
        let width = self.output_width as usize;
        match st.kind {
            ConvKind::None => return Err(Error::Internal("no colour converter")),
            ConvKind::Grayscale => {
                for r in 0..num_rows {
                    output[r][..width].copy_from_slice(&buf[0][input_row + r][..width]);
                }
            }
            ConvKind::NullRgb => {
                for r in 0..num_rows {
                    let out = &mut output[r];
                    for col in 0..width {
                        out[3 * col] = buf[0][input_row + r][col];
                        out[3 * col + 1] = buf[1][input_row + r][col];
                        out[3 * col + 2] = buf[2][input_row + r][col];
                    }
                }
            }
            ConvKind::NullCmyk => {
                for r in 0..num_rows {
                    let out = &mut output[r];
                    for col in 0..width {
                        for ci in 0..4 {
                            out[4 * col + ci] = buf[ci][input_row + r][col];
                        }
                    }
                }
            }
            ConvKind::RgbGray => {
                for r in 0..num_rows {
                    let out = &mut output[r];
                    for col in 0..width {
                        let rr = i32::from(buf[0][input_row + r][col]);
                        let g = i32::from(buf[1][input_row + r][col]);
                        let b = i32::from(buf[2][input_row + r][col]);
                        let t = &st.rgb_y_tab;
                        let v = (t[rr as usize] + t[256 + g as usize] + t[512 + b as usize])
                            >> SCALEBITS;
                        out[col] = v as u8;
                    }
                }
            }
            ConvKind::YccRgb => {
                let lay = layout(self.out_color_space).ok_or(Error::BadColorspace)?;
                for r in 0..num_rows {
                    let out = &mut output[r];
                    for col in 0..width {
                        let y = i32::from(buf[0][input_row + r][col]);
                        let cb = i32::from(buf[1][input_row + r][col]) as usize;
                        let cr = i32::from(buf[2][input_row + r][col]) as usize;
                        let o = col * lay.pixelsize;
                        out[o + lay.red] = rl.at(y + st.cr_r_tab[cr]);
                        out[o + lay.green] =
                            rl.at(y + ((st.cb_g_tab[cb] + st.cr_g_tab[cr]) >> SCALEBITS));
                        out[o + lay.blue] = rl.at(y + st.cb_b_tab[cb]);
                        if let Some(a) = lay.alpha {
                            out[o + a] = 255;
                        }
                    }
                }
            }
            ConvKind::GrayRgb => {
                let lay = layout(self.out_color_space).ok_or(Error::BadColorspace)?;
                for r in 0..num_rows {
                    let out = &mut output[r];
                    for col in 0..width {
                        let v = buf[0][input_row + r][col];
                        let o = col * lay.pixelsize;
                        out[o + lay.red] = v;
                        out[o + lay.green] = v;
                        out[o + lay.blue] = v;
                        if let Some(a) = lay.alpha {
                            out[o + a] = 255;
                        }
                    }
                }
            }
            ConvKind::RgbRgb => {
                let lay = layout(self.out_color_space).ok_or(Error::BadColorspace)?;
                for r in 0..num_rows {
                    let out = &mut output[r];
                    for col in 0..width {
                        let o = col * lay.pixelsize;
                        out[o + lay.red] = buf[0][input_row + r][col];
                        out[o + lay.green] = buf[1][input_row + r][col];
                        out[o + lay.blue] = buf[2][input_row + r][col];
                        if let Some(a) = lay.alpha {
                            out[o + a] = 255;
                        }
                    }
                }
            }
            ConvKind::YccRgb565 => {
                for r in 0..num_rows {
                    let out = &mut output[r];
                    for col in 0..width {
                        let y = i32::from(buf[0][input_row + r][col]);
                        let cb = i32::from(buf[1][input_row + r][col]) as usize;
                        let cr = i32::from(buf[2][input_row + r][col]) as usize;
                        let rr = u32::from(rl.at(y + st.cr_r_tab[cr]));
                        let g = u32::from(
                            rl.at(y + ((st.cb_g_tab[cb] + st.cr_g_tab[cr]) >> SCALEBITS)),
                        );
                        let b = u32::from(rl.at(y + st.cb_b_tab[cb]));
                        store_565(&mut out[2 * col..], pack_short_565(rr, g, b));
                    }
                }
            }
            ConvKind::GrayRgb565 => {
                for r in 0..num_rows {
                    let out = &mut output[r];
                    for col in 0..width {
                        let g = u32::from(buf[0][input_row + r][col]);
                        store_565(&mut out[2 * col..], pack_short_565(g, g, g));
                    }
                }
            }
            ConvKind::RgbRgb565 => {
                for r in 0..num_rows {
                    let out = &mut output[r];
                    for col in 0..width {
                        let rr = u32::from(buf[0][input_row + r][col]);
                        let g = u32::from(buf[1][input_row + r][col]);
                        let b = u32::from(buf[2][input_row + r][col]);
                        store_565(&mut out[2 * col..], pack_short_565(rr, g, b));
                    }
                }
            }
            ConvKind::YcckCmyk => {
                for r in 0..num_rows {
                    let out = &mut output[r];
                    for col in 0..width {
                        let y = i32::from(buf[0][input_row + r][col]);
                        let cb = i32::from(buf[1][input_row + r][col]) as usize;
                        let cr = i32::from(buf[2][input_row + r][col]) as usize;
                        let o = 4 * col;
                        out[o] = rl.at(255 - (y + st.cr_r_tab[cr]));
                        out[o + 1] =
                            rl.at(255 - (y + ((st.cb_g_tab[cb] + st.cr_g_tab[cr]) >> SCALEBITS)));
                        out[o + 2] = rl.at(255 - (y + st.cb_b_tab[cb]));
                        out[o + 3] = buf[3][input_row + r][col];
                    }
                }
            }
        }
        Ok(())
    }
}

/// `build_ycc_rgb_table`.
fn build_ycc_rgb_table(st: &mut ColorState) {
    st.cr_r_tab = vec![0; 256];
    st.cb_b_tab = vec![0; 256];
    st.cr_g_tab = vec![0; 256];
    st.cb_g_tab = vec![0; 256];
    let mut x: i32 = -128;
    for i in 0..256usize {
        // i = Cr or Cb sample, x = i - CENTERJSAMPLE
        st.cr_r_tab[i] = (fix(1.402_00) * x + ONE_HALF) >> SCALEBITS;
        st.cb_b_tab[i] = (fix(1.772_00) * x + ONE_HALF) >> SCALEBITS;
        st.cr_g_tab[i] = (-fix(0.714_14)).wrapping_mul(x);
        st.cb_g_tab[i] = (-fix(0.344_14)).wrapping_mul(x).wrapping_add(ONE_HALF);
        x += 1;
    }
}

/// `build_rgb_y_table`.
fn build_rgb_y_table(st: &mut ColorState) {
    st.rgb_y_tab = vec![0; 3 * 256];
    for i in 0..256usize {
        let i = i as i32;
        st.rgb_y_tab[i as usize] = fix(0.299_00).wrapping_mul(i);
        st.rgb_y_tab[256 + i as usize] = fix(0.587_00).wrapping_mul(i);
        st.rgb_y_tab[512 + i as usize] = fix(0.114_00).wrapping_mul(i).wrapping_add(ONE_HALF);
    }
}

// Copyright (C) 1998-2025 Glenn Randers-Pehrson and the libpng contributors.
// Use of this source code is governed by the libpng licence (libpng-2.0) in the LICENSE file.
// Port of: pngrtran.c (`png_set_strip_16`, `png_set_expand_gray_1_2_4_to_8`,
// `png_set_tRNS_to_alpha`, `png_read_transform_info`, `png_do_read_transformations`,
// `png_do_expand`, `png_do_unpack`, `png_do_chop`, `png_read_start_row`), and pngtrans.c
// (`png_set_packing`, `png_set_interlace_handling`), libpng 1.6.56,
// skia.googlesource.com/third_party/libpng@d5515b5b.
//
// Scope: these are the transforms `SkPngCodec` can enable. Gamma, background compositing,
// RGB-to-gray, palette expansion, the alpha and filler transforms, and the user transform have no
// public setter in this port, so their branches are unreachable and not ported.

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
use crate::error::PngResult;
use crate::interlace::PNG_INTERLACE;
use crate::structs::{PngColor16, PngInfo, PngStruct, flag, rowbytes};

/// Port of the `PNG_*` transform bits (pngpriv.h) that the ported transforms use.
pub(crate) const PNG_PACK: u32 = 0x0004;
pub(crate) const PNG_16_TO_8: u32 = 0x0400;
pub(crate) const PNG_EXPAND: u32 = 0x1000;
pub(crate) const PNG_EXPAND_TRNS: u32 = 0x0200_0000;

/// Port of `PNG_COLOR_TYPE_*`.
const COLOR_GRAY: u8 = 0;
const COLOR_RGB: u8 = 2;
const COLOR_PALETTE: u8 = 3;
const COLOR_GRAY_ALPHA: u8 = 4;
const COLOR_RGB_ALPHA: u8 = 6;
const COLOR_MASK_COLOR: u8 = 2;
const COLOR_MASK_ALPHA: u8 = 4;

/// Port of `png_row_info` (png.h): the row's shape as the transforms see it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RowInfo {
    pub width: u32,
    pub rowbytes: usize,
    pub color_type: u8,
    pub bit_depth: u8,
    pub channels: u8,
    pub pixel_depth: u8,
}

impl PngStruct {
    /// Port of `png_set_strip_16` (pngrtran.c#L112-L118): 16-bit samples keep their high byte.
    #[doc(alias = "png_set_strip_16")]
    pub fn set_strip_16(&mut self) {
        self.transformations |= PNG_16_TO_8;
    }

    /// Port of `png_set_packing` (pngtrans.c#L19-L27): sub-byte samples are unpacked to bytes.
    #[doc(alias = "png_set_packing")]
    pub fn set_packing(&mut self) {
        self.transformations |= PNG_PACK;
    }

    /// Port of `png_set_expand_gray_1_2_4_to_8` (pngrtran.c#L573-L579).
    #[doc(alias = "png_set_expand_gray_1_2_4_to_8")]
    pub fn set_expand_gray_1_2_4_to_8(&mut self) {
        self.transformations |= PNG_EXPAND;
    }

    /// Port of `png_set_tRNS_to_alpha` (pngrtran.c#L581-L587): transparency becomes an alpha channel.
    #[doc(alias = "png_set_tRNS_to_alpha")]
    pub fn set_trns_to_alpha(&mut self) {
        self.transformations |= PNG_EXPAND | PNG_EXPAND_TRNS;
    }

    /// Port of `png_set_interlace_handling` (pngtrans.c#L73-L82). Returns the number of passes the
    /// caller must read: 7 for an interlaced image, 1 otherwise.
    #[doc(alias = "png_set_interlace_handling")]
    pub fn set_interlace_handling(&mut self) -> u32 {
        if self.interlaced != 0 {
            self.transformations |= PNG_INTERLACE;
            7
        } else {
            1
        }
    }

    /// Port of `png_read_transform_info` (pngrtran.c#L1159-L1263). Works out the format of the
    /// rows the transforms produce, and stores it in `info`.
    #[doc(alias = "png_read_transform_info")]
    pub(crate) fn read_transform_info(&mut self, info: &mut PngInfo) -> PngResult<()> {
        let t = self.transformations;
        if t & PNG_EXPAND != 0 {
            if info.color_type == COLOR_PALETTE {
                // Not reachable: no public setter enables EXPAND for palette images.
                info.color_type = if self.num_trans > 0 {
                    COLOR_RGB_ALPHA
                } else {
                    COLOR_RGB
                };
                info.bit_depth = 8;
                info.num_trans = 0;
                if info.palette.is_empty() {
                    return Err(self.error("Palette is NULL in indexed image"));
                }
            } else {
                if self.num_trans != 0 && t & PNG_EXPAND_TRNS != 0 {
                    info.color_type |= COLOR_MASK_ALPHA;
                }
                if info.bit_depth < 8 {
                    info.bit_depth = 8;
                }
                info.num_trans = 0;
            }
        }
        if info.bit_depth == 16 && t & PNG_16_TO_8 != 0 {
            info.bit_depth = 8;
        }
        if t & PNG_PACK != 0 && info.bit_depth < 8 {
            info.bit_depth = 8;
        }
        info.channels = if info.color_type == COLOR_PALETTE {
            1
        } else if info.color_type & COLOR_MASK_COLOR != 0 {
            3
        } else {
            1
        };
        if info.color_type & COLOR_MASK_ALPHA != 0 {
            info.channels += 1;
        }
        info.pixel_depth = info.channels * info.bit_depth;
        info.rowbytes = rowbytes(info.pixel_depth, info.width);
        self.info_rowbytes = info.rowbytes;
        Ok(())
    }

    /// Port of `png_do_read_transformations` (pngrtran.c#L3252-L3374), for the transforms that
    /// are reachable. `row` is `row_buf + 1`, of the row described by `row_info`.
    #[doc(alias = "png_do_read_transformations")]
    pub(crate) fn do_read_transformations(&mut self, row_info: &mut RowInfo, row: &mut [u8]) {
        let t = self.transformations;
        if t & PNG_EXPAND != 0 && row_info.color_type != COLOR_PALETTE {
            let trans = if self.num_trans != 0 && t & PNG_EXPAND_TRNS != 0 {
                Some(self.trans_color)
            } else {
                None
            };
            do_expand(row_info, row, trans);
        }
        if t & PNG_16_TO_8 != 0 {
            do_chop(row_info, row);
        }
        if t & PNG_PACK != 0 {
            do_unpack(row_info, row);
        }
    }

    /// Port of `png_read_start_row` (pngrutil.c#L2544-L2702) for the progressive read. Fixes the
    /// pass sizes, sizes the row buffers for the widest format the transforms produce, and claims
    /// the inflate stream for IDAT.
    #[doc(alias = "png_read_start_row")]
    pub(crate) fn read_start_row(&mut self) -> PngResult<()> {
        if self.interlaced != 0 {
            if self.transformations & PNG_INTERLACE == 0 {
                self.num_rows = self.height.div_ceil(8);
            } else {
                self.num_rows = self.height;
            }
            // png_read_start_row (pngrutil.c): (width + inc - 1 - start) / inc for this pass.
            self.iwidth = (self.width + crate::interlace::PASS_INC[self.pass as usize]
                - 1
                - PASS_START[self.pass as usize])
                / crate::interlace::PASS_INC[self.pass as usize];
        } else {
            self.num_rows = self.height;
            self.iwidth = self.width;
        }
        let mut max_pixel_depth = u32::from(self.pixel_depth);
        if self.transformations & PNG_PACK != 0 && self.bit_depth < 8 {
            max_pixel_depth = 8;
        }
        if self.transformations & PNG_EXPAND != 0 {
            if self.color_type == COLOR_PALETTE {
                max_pixel_depth = if self.num_trans != 0 { 32 } else { 24 };
            } else if self.color_type == COLOR_GRAY {
                if max_pixel_depth < 8 {
                    max_pixel_depth = 8;
                }
                if self.num_trans != 0 {
                    max_pixel_depth *= 2;
                }
            } else if self.color_type == COLOR_RGB && self.num_trans != 0 {
                max_pixel_depth *= 4;
                max_pixel_depth /= 3;
            }
        }
        self.maximum_pixel_depth = max_pixel_depth as u8;
        self.transformed_pixel_depth = 0;
        let width8 = (self.width as usize + 7) & !7usize;
        let row_bytes = rowbytes(max_pixel_depth as u8, width8 as u32)
            + 1
            + ((max_pixel_depth as usize + 7) >> 3);
        self.row_buf = vec![0u8; row_bytes + 48];
        self.prev_row = vec![0u8; row_bytes + 48];
        // memset(prev_row, 0, rowbytes + 1)
        self.prev_row[..=self.rowbytes].fill(0);
        if self.inflate_claim(crate::structs::PNG_IDAT) != skia_rust_zlib::ReturnCode::Ok {
            let msg = self.zstream_msg.unwrap_or("zlib error").to_owned();
            return Err(self.error(&msg));
        }
        self.flags |= flag::ROW_INIT;
        Ok(())
    }

    /// Port of `png_read_update_info` (pngread.c#L80-L95): starts the rows and fixes the output
    /// format in `info`. Calling it again is an error in C, which the codec never does.
    #[doc(alias = "png_read_update_info")]
    pub fn read_update_info(&mut self, info: &mut PngInfo) -> PngResult<()> {
        if self.flags & flag::ROW_INIT == 0 {
            self.read_start_row()?;
            self.read_transform_info(info)?;
        }
        Ok(())
    }
}

/// Port of `png_pass_start` (pngpread.c): the first column of each Adam7 pass.
const PASS_START: [u32; 7] = [0, 4, 0, 2, 0, 1, 0];

/// Port of `png_do_expand` (pngrtran.c#L2965-L3159) for gray and RGB images. `trans` is the
/// tRNS colour key, when transparency becomes alpha. The expansion runs from the end of the row,
/// as in C, so each source byte is read before it is overwritten.
#[doc(alias = "png_do_expand")]
pub(crate) fn do_expand(row_info: &mut RowInfo, row: &mut [u8], trans: Option<PngColor16>) {
    let row_width = row_info.width as usize;
    if row_info.color_type == COLOR_GRAY {
        let mut gray: u32 = trans.map_or(0, |t| u32::from(t.gray));
        if row_info.bit_depth < 8 {
            match row_info.bit_depth {
                1 => {
                    gray = (gray & 0x01) * 0xff;
                    let mut sp = (row_width - 1) >> 3;
                    let mut dp = row_width - 1;
                    let mut shift: i32 = 7 - ((row_width + 7) & 0x07) as i32;
                    for _ in 0..row_width {
                        row[dp] = if (u32::from(row[sp]) >> shift) & 0x01 != 0 {
                            0xff
                        } else {
                            0
                        };
                        if shift == 7 {
                            shift = 0;
                            sp = sp.wrapping_sub(1);
                        } else {
                            shift += 1;
                        }
                        dp = dp.wrapping_sub(1);
                    }
                }
                2 => {
                    gray = (gray & 0x03) * 0x55;
                    let mut sp = (row_width - 1) >> 2;
                    let mut dp = row_width - 1;
                    let mut shift: i32 = (3 - ((row_width + 3) & 0x03) as i32) << 1;
                    for _ in 0..row_width {
                        let value = (u32::from(row[sp]) >> shift) & 0x03;
                        row[dp] = (value | (value << 2) | (value << 4) | (value << 6)) as u8;
                        if shift == 6 {
                            shift = 0;
                            sp = sp.wrapping_sub(1);
                        } else {
                            shift += 2;
                        }
                        dp = dp.wrapping_sub(1);
                    }
                }
                4 => {
                    gray = (gray & 0x0f) * 0x11;
                    let mut sp = (row_width - 1) >> 1;
                    let mut dp = row_width - 1;
                    let mut shift: i32 = (1 - ((row_width + 1) & 0x01) as i32) << 2;
                    for _ in 0..row_width {
                        let value = (u32::from(row[sp]) >> shift) & 0x0f;
                        row[dp] = (value | (value << 4)) as u8;
                        if shift == 4 {
                            shift = 0;
                            sp = sp.wrapping_sub(1);
                        } else {
                            shift = 4;
                        }
                        dp = dp.wrapping_sub(1);
                    }
                }
                _ => {}
            }
            row_info.bit_depth = 8;
            row_info.pixel_depth = 8;
            row_info.rowbytes = row_width;
        }
        if let Some(t) = trans {
            if row_info.bit_depth == 8 {
                let gray = (gray & 0xff) as u8;
                let mut sp = row_width - 1;
                let mut dp = (row_width << 1) - 1;
                for _ in 0..row_width {
                    row[dp] = if row[sp] == gray { 0 } else { 0xff };
                    dp = dp.wrapping_sub(1);
                    row[dp] = row[sp];
                    sp = sp.wrapping_sub(1);
                    dp = dp.wrapping_sub(1);
                }
            } else if row_info.bit_depth == 16 {
                let gray_high = ((gray >> 8) & 0xff) as u8;
                let gray_low = (gray & 0xff) as u8;
                let _ = t;
                let mut sp = row_info.rowbytes - 1;
                let mut dp = (row_info.rowbytes << 1) - 1;
                for _ in 0..row_width {
                    let hit = row[sp - 1] == gray_high && row[sp] == gray_low;
                    let v = if hit { 0 } else { 0xff };
                    row[dp] = v;
                    dp = dp.wrapping_sub(1);
                    row[dp] = v;
                    dp = dp.wrapping_sub(1);
                    row[dp] = row[sp];
                    sp = sp.wrapping_sub(1);
                    dp = dp.wrapping_sub(1);
                    row[dp] = row[sp];
                    sp = sp.wrapping_sub(1);
                    dp = dp.wrapping_sub(1);
                }
            }
            row_info.color_type = COLOR_GRAY_ALPHA;
            row_info.channels = 2;
            row_info.pixel_depth = row_info.bit_depth << 1;
            row_info.rowbytes = rowbytes(row_info.pixel_depth, row_width as u32);
        }
    } else if row_info.color_type == COLOR_RGB
        && let Some(t) = trans
    {
        if row_info.bit_depth == 8 {
            let red = (t.red & 0xff) as u8;
            let green = (t.green & 0xff) as u8;
            let blue = (t.blue & 0xff) as u8;
            let mut sp = row_info.rowbytes - 1;
            let mut dp = (row_width << 2) - 1;
            for _ in 0..row_width {
                let hit = row[sp - 2] == red && row[sp - 1] == green && row[sp] == blue;
                row[dp] = if hit { 0 } else { 0xff };
                dp = dp.wrapping_sub(1);
                row[dp] = row[sp];
                sp = sp.wrapping_sub(1);
                dp = dp.wrapping_sub(1);
                row[dp] = row[sp];
                sp = sp.wrapping_sub(1);
                dp = dp.wrapping_sub(1);
                row[dp] = row[sp];
                sp = sp.wrapping_sub(1);
                dp = dp.wrapping_sub(1);
            }
        } else if row_info.bit_depth == 16 {
            let rh = ((t.red >> 8) & 0xff) as u8;
            let gh = ((t.green >> 8) & 0xff) as u8;
            let bh = ((t.blue >> 8) & 0xff) as u8;
            let rl = (t.red & 0xff) as u8;
            let gl = (t.green & 0xff) as u8;
            let bl = (t.blue & 0xff) as u8;
            let mut sp = row_info.rowbytes - 1;
            let mut dp = (row_width << 3) - 1;
            for _ in 0..row_width {
                let hit = row[sp - 5] == rh
                    && row[sp - 4] == rl
                    && row[sp - 3] == gh
                    && row[sp - 2] == gl
                    && row[sp - 1] == bh
                    && row[sp] == bl;
                let v = if hit { 0 } else { 0xff };
                row[dp] = v;
                dp = dp.wrapping_sub(1);
                row[dp] = v;
                dp = dp.wrapping_sub(1);
                for _ in 0..6 {
                    row[dp] = row[sp];
                    sp = sp.wrapping_sub(1);
                    dp = dp.wrapping_sub(1);
                }
            }
        }
        row_info.color_type = COLOR_RGB_ALPHA;
        row_info.channels = 4;
        row_info.pixel_depth = row_info.bit_depth << 2;
        row_info.rowbytes = rowbytes(row_info.pixel_depth, row_width as u32);
    }
}

/// Port of `png_do_unpack` (pngrtran.c#L1265-L1331): sub-byte samples become one byte each, from
/// the end of the row.
#[doc(alias = "png_do_unpack")]
pub(crate) fn do_unpack(row_info: &mut RowInfo, row: &mut [u8]) {
    if row_info.bit_depth < 8 {
        let row_width = row_info.width as usize;
        match row_info.bit_depth {
            1 => {
                let mut sp = (row_width - 1) >> 3;
                let mut dp = row_width - 1;
                let mut shift: i32 = 7 - ((row_width + 7) & 0x07) as i32;
                for _ in 0..row_width {
                    row[dp] = ((u32::from(row[sp]) >> shift) & 0x01) as u8;
                    if shift == 7 {
                        shift = 0;
                        sp = sp.wrapping_sub(1);
                    } else {
                        shift += 1;
                    }
                    dp = dp.wrapping_sub(1);
                }
            }
            2 => {
                let mut sp = (row_width - 1) >> 2;
                let mut dp = row_width - 1;
                let mut shift: i32 = (3 - ((row_width + 3) & 0x03) as i32) << 1;
                for _ in 0..row_width {
                    row[dp] = ((u32::from(row[sp]) >> shift) & 0x03) as u8;
                    if shift == 6 {
                        shift = 0;
                        sp = sp.wrapping_sub(1);
                    } else {
                        shift += 2;
                    }
                    dp = dp.wrapping_sub(1);
                }
            }
            4 => {
                let mut sp = (row_width - 1) >> 1;
                let mut dp = row_width - 1;
                let mut shift: i32 = (1 - ((row_width + 1) & 0x01) as i32) << 2;
                for _ in 0..row_width {
                    row[dp] = ((u32::from(row[sp]) >> shift) & 0x0f) as u8;
                    if shift == 4 {
                        shift = 0;
                        sp = sp.wrapping_sub(1);
                    } else {
                        shift = 4;
                    }
                    dp = dp.wrapping_sub(1);
                }
            }
            _ => {}
        }
        row_info.bit_depth = 8;
        row_info.pixel_depth = 8 * row_info.channels;
        row_info.rowbytes = row_width * usize::from(row_info.channels);
    }
}

/// Port of `png_do_chop` (pngrtran.c#L1460-L1477): 16-bit samples keep their high byte.
#[doc(alias = "png_do_chop")]
pub(crate) fn do_chop(row_info: &mut RowInfo, row: &mut [u8]) {
    if row_info.bit_depth == 16 {
        let ep = row_info.rowbytes;
        let mut sp = 0usize;
        let mut dp = 0usize;
        while sp < ep {
            row[dp] = row[sp];
            dp += 1;
            sp += 2;
        }
        row_info.bit_depth = 8;
        row_info.pixel_depth = 8 * row_info.channels;
        row_info.rowbytes = row_info.width as usize * usize::from(row_info.channels);
    }
}

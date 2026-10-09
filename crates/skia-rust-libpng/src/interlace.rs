// Copyright (C) 1998-2025 Glenn Randers-Pehrson and the libpng contributors.
// Use of this source code is governed by the libpng licence (libpng-2.0) in the LICENSE file.
// Port of: pngrutil.c#L1837-L1985 (`png_combine_row`), pngrutil.c#L2071-L2262
// (`png_do_read_interlace`), and the Adam7 macros of png.h#L2685-L2725 (libpng 1.6.56,
// skia.googlesource.com/third_party/libpng@d5515b5b).

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
use crate::structs::{PngStruct, rowbytes};

/// Port of `PNG_INTERLACE` and `PNG_PACKSWAP` (pngpriv.h transform bits).
pub(crate) const PNG_INTERLACE: u32 = 0x0002;
pub(crate) const PNG_PACKSWAP: u32 = 0x0001_0000;

/// Port of `PNG_PASS_START_COL(pass)` (png.h#L2685).
#[must_use]
pub(crate) const fn pass_start_col(pass: u32) -> u32 {
    ((1 & pass) << (3 - ((pass + 1) >> 1))) & 7
}

/// Port of `PNG_PASS_COL_OFFSET(pass)` (png.h#L2693).
#[must_use]
pub(crate) const fn pass_col_offset(pass: u32) -> u32 {
    1 << ((7 - pass) >> 1)
}

/// Port of `png_pass_inc` (pngpread.c): the column step of each Adam7 pass.
pub(crate) const PASS_INC: [u32; 7] = [8, 8, 4, 4, 2, 2, 1];

/// Port of `PNG_LSR` and `PNG_LSL` (pngrutil.c#L1866-L1867): shifts with the count masked to 5
/// bits, as in C.
const fn lsr(x: u32, s: u32) -> u32 {
    x >> (s & 0x1f)
}
const fn lsl(x: u32, s: u32) -> u32 {
    x << (s & 0x1f)
}

/// Port of `S_COPY(p,x)` (pngrutil.c#L1868-L1870).
const fn s_copy(p: u32, x: u32) -> u32 {
    (if p < 4 {
        lsr(0x8008_8822, (3 - p) * 8 + (7 - x))
    } else {
        lsr(0xaa55_ff00, (7 - p) * 8 + (7 - x))
    }) & 1
}

/// Port of `B_COPY(p,x)` (pngrutil.c#L1871-L1873).
const fn b_copy(p: u32, x: u32) -> u32 {
    (if p < 4 {
        lsr(0xff0f_ff33, (3 - p) * 8 + (7 - x))
    } else {
        lsr(0xff55_ff00, (7 - p) * 8 + (7 - x))
    }) & 1
}

/// Port of `PIXEL_MASK(p,x,d,s)` (pngrutil.c#L1874).
const fn pixel_mask(x: u32, d: u32, s: u32) -> u32 {
    lsl(lsl(1, d) - 1, (x * d) ^ (if s != 0 { 8 - d } else { 0 }))
}

/// Port of `MASK_EXPAND(m,d)` (pngrutil.c#L1878).
const fn mask_expand(m: u32, d: u32) -> u32 {
    m * (if d == 1 {
        0x0101_0101
    } else if d == 2 {
        0x0001_0001
    } else {
        1
    })
}

/// Port of `S_MASK(p,d,s)` (pngrutil.c#L1880): the source-pixel bits that pass `p` copies.
const fn s_mask(p: u32, d: u32, s: u32) -> u32 {
    let mut sum = 0;
    let mut x = 0;
    while x < 8 {
        if s_copy(p, x) != 0 {
            sum += pixel_mask(x, d, s);
        }
        x += 1;
    }
    mask_expand(sum, d)
}

/// Port of `B_MASK(p,d,s)` (pngrutil.c#L1881): the display (every-other-pixel) mask.
const fn b_mask(p: u32, d: u32, s: u32) -> u32 {
    let mut sum = 0;
    let mut x = 0;
    while x < 8 {
        if b_copy(p, x) != 0 {
            sum += pixel_mask(x, d, s);
        }
        x += 1;
    }
    mask_expand(sum, d)
}

/// Port of `row_mask[2][3][6]` (pngrutil.c#L1893-L1898), indexed `[s][depth index][pass]`, with
/// depth indices 0, 1, 2 for bit depths 1, 2, 4.
const ROW_MASK: [[[u32; 6]; 3]; 2] = {
    const DEPTHS: [u32; 3] = [1, 2, 4];
    let mut out = [[[0u32; 6]; 3]; 2];
    let mut s = 0;
    while s < 2 {
        let mut di = 0;
        while di < 3 {
            let mut p = 0;
            while p < 6 {
                out[s][di][p] = s_mask(p as u32, DEPTHS[di], s as u32);
                p += 1;
            }
            di += 1;
        }
        s += 1;
    }
    out
};

/// Port of `display_mask[2][3][3]` (pngrutil.c#L1899-L1904), indexed like [`ROW_MASK`], with the
/// odd passes 1, 3, 5 at `pass >> 1`.
const DISPLAY_MASK: [[[u32; 3]; 3]; 2] = {
    const DEPTHS: [u32; 3] = [1, 2, 4];
    let mut out = [[[0u32; 3]; 3]; 2];
    let mut s = 0;
    while s < 2 {
        let mut di = 0;
        while di < 3 {
            let mut k = 0;
            while k < 3 {
                out[s][di][k] = b_mask((2 * k + 1) as u32, DEPTHS[di], s as u32);
                k += 1;
            }
            di += 1;
        }
        s += 1;
    }
    out
};

/// Port of `DEPTH_INDEX(d)` (pngrutil.c#L1905).
const fn depth_index(d: u32) -> usize {
    if d == 1 {
        0
    } else if d == 2 {
        1
    } else {
        2
    }
}

/// Port of `MASK(pass,depth,display,png)` (pngrutil.c#L1906-L1907).
fn mask_for(pass: u32, depth: u32, display: bool, png: usize) -> u32 {
    if display {
        DISPLAY_MASK[png][depth_index(depth)][(pass >> 1) as usize]
    } else {
        ROW_MASK[png][depth_index(depth)][pass as usize]
    }
}

/// Port of `png_do_read_interlace` (pngrutil.c#L2071-L2262). Expands the row of the current pass
/// to the full row width, replicating each pixel as Adam7 requires, and updates the row's width
/// and rowbytes.
#[doc(alias = "png_do_read_interlace")]
pub(crate) fn do_read_interlace(
    row: &mut [u8],
    width: &mut u32,
    rowbytes_out: &mut usize,
    pixel_depth: u8,
    pass: u32,
    transformations: u32,
) {
    let final_width = *width * PASS_INC[pass as usize];
    let jstop = PASS_INC[pass as usize] as usize;
    let w = *width as usize;
    let fw = final_width as usize;
    let packswap = transformations & PNG_PACKSWAP != 0;
    match pixel_depth {
        1 | 2 | 4 => {
            // Bit depths 1, 2 and 4 (pngrutil.c#L2083-L2226): each sample is shifted into place.
            let d = i32::from(pixel_depth);
            let ppb = (8 / d) as usize;
            let (keep_const, keep_base, value_mask): (u32, i32, u32) = match pixel_depth {
                1 => (0x7f7f, 7, 0x01),
                2 => (0x3f3f, 6, 0x03),
                _ => (0x0f0f, 4, 0x0f),
            };
            let mut sp = (w - 1) / ppb;
            let mut dp = (fw - 1) / ppb;
            let span = (ppb as i32 - 1) * d;
            let (mut sshift, mut dshift, s_start, s_end, s_inc): (i32, i32, i32, i32, i32);
            if packswap {
                sshift = ((w + ppb - 1) % ppb) as i32 * d;
                dshift = ((fw + ppb - 1) % ppb) as i32 * d;
                s_start = span;
                s_end = 0;
                s_inc = -d;
            } else {
                sshift = span - ((w + ppb - 1) % ppb) as i32 * d;
                dshift = span - ((fw + ppb - 1) % ppb) as i32 * d;
                s_start = 0;
                s_end = span;
                s_inc = d;
            }
            for _ in 0..w {
                let v = (u32::from(row[sp]) >> sshift as u32) & value_mask;
                for _ in 0..jstop {
                    let keep = keep_const >> (keep_base - dshift) as u32;
                    let tmp = (u32::from(row[dp]) & keep) | (v << dshift as u32);
                    row[dp] = tmp as u8;
                    if dshift == s_end {
                        dshift = s_start;
                        dp = dp.wrapping_sub(1);
                    } else {
                        dshift += s_inc;
                    }
                }
                if sshift == s_end {
                    sshift = s_start;
                    sp = sp.wrapping_sub(1);
                } else {
                    sshift += s_inc;
                }
            }
        }
        _ => {
            // Whole pixels (pngrutil.c#L2228-L2251): copy each pixel `jstop` times, from the end.
            let pixel_bytes = usize::from(pixel_depth >> 3);
            let mut sp = (w - 1) * pixel_bytes;
            let mut dp = (fw - 1) * pixel_bytes;
            for _ in 0..w {
                let mut v = [0u8; 8];
                v[..pixel_bytes].copy_from_slice(&row[sp..sp + pixel_bytes]);
                for _ in 0..jstop {
                    row[dp..dp + pixel_bytes].copy_from_slice(&v[..pixel_bytes]);
                    dp = dp.wrapping_sub(pixel_bytes);
                }
                sp = sp.wrapping_sub(pixel_bytes);
            }
        }
    }
    *width = final_width;
    *rowbytes_out = rowbytes(pixel_depth, final_width);
}

impl PngStruct {
    /// Port of `png_combine_row` (pngrutil.c#L1837-L1985). Copies the pixels of the current pass
    /// (`row_buf + 1`) into `dp`, the full-width row, at their Adam7 positions. `display` is 1 for
    /// the progressive combine and 0 for a plain copy.
    #[doc(alias = "png_combine_row")]
    pub(crate) fn combine_row(&mut self, dp: &mut [u8], display: i32) -> PngResult<()> {
        let pixel_depth = u32::from(self.transformed_pixel_depth);
        let row_width0 = self.width as usize;
        let pass = self.pass;
        if pixel_depth == 0 {
            return Err(self.error("internal row logic error"));
        }
        if self.info_rowbytes != 0
            && self.info_rowbytes != rowbytes(pixel_depth as u8, row_width0 as u32)
        {
            return Err(self.error("internal row size calculation error"));
        }
        if row_width0 == 0 {
            return Err(self.error("internal row width error"));
        }
        let mut end_ptr: Option<usize> = None;
        let mut end_byte: u8 = 0;
        let mut end_mask: u32 = 0;
        if (pixel_depth as usize * row_width0) & 7 != 0 {
            let p = rowbytes(pixel_depth as u8, row_width0 as u32) - 1;
            end_ptr = Some(p);
            end_byte = dp[p];
            let shift = (pixel_depth as usize * row_width0) & 7;
            end_mask = if self.transformations & PNG_PACKSWAP != 0 {
                0xffu32 << shift
            } else {
                0xffu32 >> shift
            };
        }
        let display_pass = self.interlaced != 0
            && self.transformations & PNG_INTERLACE != 0
            && pass < 6
            && (display == 0 || (display == 1 && (pass & 1) != 0));
        if display_pass {
            if row_width0 as u32 <= pass_start_col(pass) {
                return Ok(());
            }
            if pixel_depth < 8 {
                let mut row_width = row_width0;
                let pixels_per_byte = (8 / pixel_depth) as usize;
                let packswap = self.transformations & PNG_PACKSWAP != 0;
                let mut mask = if packswap {
                    mask_for(pass, pixel_depth, display != 0, 0)
                } else {
                    mask_for(pass, pixel_depth, display != 0, 1)
                };
                let mut i = 0usize;
                loop {
                    let m_full = mask;
                    mask = m_full.rotate_right(8);
                    let m = (m_full & 0xff) as u8;
                    if m != 0 {
                        let sv = self.row_buf[1 + i];
                        if m == 0xff {
                            dp[i] = sv;
                        } else {
                            dp[i] = (dp[i] & !m) | (sv & m);
                        }
                    }
                    if row_width <= pixels_per_byte {
                        break;
                    }
                    row_width -= pixels_per_byte;
                    i += 1;
                }
            } else {
                // Whole pixels. Every path in C returns from here, so the end byte is not updated.
                if pixel_depth & 7 != 0 {
                    return Err(self.error("invalid user transform pixel depth"));
                }
                let pd = (pixel_depth >> 3) as usize;
                let offset = pass_start_col(pass) as usize * pd;
                let mut row_width = row_width0 * pd - offset;
                let mut dpos = offset;
                let mut spos = 1 + offset;
                let mut bytes_to_copy = if display != 0 {
                    let b = (1usize << ((6 - pass) >> 1)) * pd;
                    core::cmp::min(b, row_width)
                } else {
                    pd
                };
                let bytes_to_jump = pass_col_offset(pass) as usize * pd;
                loop {
                    let n = core::cmp::min(bytes_to_copy, row_width);
                    dp[dpos..dpos + n].copy_from_slice(&self.row_buf[spos..spos + n]);
                    if row_width <= bytes_to_jump {
                        return Ok(());
                    }
                    spos += bytes_to_jump;
                    dpos += bytes_to_jump;
                    row_width -= bytes_to_jump;
                    if bytes_to_copy > row_width {
                        bytes_to_copy = row_width;
                    }
                }
            }
        } else {
            let n = rowbytes(pixel_depth as u8, row_width0 as u32);
            dp[..n].copy_from_slice(&self.row_buf[1..=n]);
        }
        if let Some(p) = end_ptr {
            let keep = u32::from(dp[p]) & !end_mask;
            dp[p] = ((u32::from(end_byte) & end_mask) | keep) as u8;
        }
        Ok(())
    }
}

// Copyright (C) 1998-2025 Glenn Randers-Pehrson and the libpng contributors.
// Use of this source code is governed by the libpng licence (libpng-2.0) in the LICENSE file.
// Port of: pngrutil.c#L1219-L1350 (`png_handle_pCAL`, `png_handle_sCAL`), pngrutil.c#L834-L925
// (`png_handle_sPLT`), and png.c#L1102-L1172 (`png_check_fp_number`), libpng 1.6.56,
// skia.googlesource.com/third_party/libpng@d5515b5b.
//
// The values of these chunks are validated exactly as libpng validates them, which decides
// whether a chunk is an error. The values themselves are not kept: Skia's codec never reads them.

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
use crate::rutil::{HandleResult, get_int_32};
use crate::structs::{PngInfo, PngStruct};

/// Port of `PNG_FP_*` (pngpriv.h#L2114-L2153): the state of the floating-point parser.
const PNG_FP_INTEGER: i32 = 0;
const PNG_FP_FRACTION: i32 = 1;
const PNG_FP_EXPONENT: i32 = 2;
const PNG_FP_STATE: i32 = 3;
const PNG_FP_SAW_SIGN: i32 = 4;
const PNG_FP_SAW_DIGIT: i32 = 8;
const PNG_FP_SAW_DOT: i32 = 16;
const PNG_FP_SAW_E: i32 = 32;
const PNG_FP_SAW_ANY: i32 = 60;
const PNG_FP_WAS_VALID: i32 = 64;
const PNG_FP_NEGATIVE: i32 = 128;
const PNG_FP_NONZERO: i32 = 256;
const PNG_FP_STICKY: i32 = 448;
const PNG_FP_NZ_MASK: i32 = PNG_FP_SAW_DIGIT | PNG_FP_NEGATIVE | PNG_FP_NONZERO;
const PNG_FP_Z_MASK: i32 = PNG_FP_SAW_DIGIT | PNG_FP_NONZERO;

/// Port of `PNG_FP_IS_POSITIVE(state)` (pngpriv.h#L2153).
#[must_use]
pub(crate) const fn fp_is_positive(state: i32) -> bool {
    (state & PNG_FP_NZ_MASK) == PNG_FP_Z_MASK
}

/// Port of `PNG_EQUATION_*` (png.h): the pCAL equation types.
const PNG_EQUATION_LINEAR: u8 = 0;
const PNG_EQUATION_BASE_E: u8 = 1;
const PNG_EQUATION_ARBITRARY: u8 = 2;
const PNG_EQUATION_HYPERBOLIC: u8 = 3;
const PNG_EQUATION_LAST: u8 = 4;

/// Port of `png_check_fp_number` (png.c#L1102-L1172): scans a decimal floating-point number
/// starting at `*whereami`, updating `state` and `whereami`. Returns whether a digit was seen.
#[must_use]
pub(crate) fn check_fp_number(
    string: &[u8],
    size: usize,
    state: &mut i32,
    whereami: &mut usize,
) -> bool {
    let mut st = *state;
    let mut i = *whereami;
    'scan: while i < size {
        let c = string[i];
        let typ: i32 = match c {
            43 => PNG_FP_SAW_SIGN,
            45 => PNG_FP_SAW_SIGN + PNG_FP_NEGATIVE,
            46 => PNG_FP_SAW_DOT,
            48 => PNG_FP_SAW_DIGIT,
            49..=57 => PNG_FP_SAW_DIGIT + PNG_FP_NONZERO,
            69 | 101 => PNG_FP_SAW_E,
            _ => break 'scan,
        };
        let key = (st & PNG_FP_STATE) + (typ & PNG_FP_SAW_ANY);
        match key {
            k if k == PNG_FP_INTEGER + PNG_FP_SAW_SIGN => {
                if st & PNG_FP_SAW_ANY != 0 {
                    break 'scan;
                }
                st |= typ;
            }
            k if k == PNG_FP_INTEGER + PNG_FP_SAW_DOT => {
                if st & PNG_FP_SAW_DOT != 0 {
                    break 'scan;
                } else if st & PNG_FP_SAW_DIGIT != 0 {
                    st |= typ;
                } else {
                    st = (PNG_FP_FRACTION | typ) | (st & PNG_FP_STICKY);
                }
            }
            k if k == PNG_FP_INTEGER + PNG_FP_SAW_DIGIT => {
                if st & PNG_FP_SAW_DOT != 0 {
                    st = (PNG_FP_FRACTION | PNG_FP_SAW_DOT) | (st & PNG_FP_STICKY);
                }
                st |= typ | PNG_FP_WAS_VALID;
            }
            k if k == PNG_FP_INTEGER + PNG_FP_SAW_E => {
                if st & PNG_FP_SAW_DIGIT == 0 {
                    break 'scan;
                }
                st = PNG_FP_EXPONENT | (st & PNG_FP_STICKY);
            }
            k if k == PNG_FP_FRACTION + PNG_FP_SAW_DIGIT => {
                st |= typ | PNG_FP_WAS_VALID;
            }
            k if k == PNG_FP_FRACTION + PNG_FP_SAW_E => {
                if st & PNG_FP_SAW_DIGIT == 0 {
                    break 'scan;
                }
                st = PNG_FP_EXPONENT | (st & PNG_FP_STICKY);
            }
            k if k == PNG_FP_EXPONENT + PNG_FP_SAW_SIGN => {
                if st & PNG_FP_SAW_ANY != 0 {
                    break 'scan;
                }
                st |= PNG_FP_SAW_SIGN;
            }
            k if k == PNG_FP_EXPONENT + PNG_FP_SAW_DIGIT => {
                st |= PNG_FP_SAW_DIGIT | PNG_FP_WAS_VALID;
            }
            _ => break 'scan,
        }
        i += 1;
    }
    *state = st;
    *whereami = i;
    st & PNG_FP_SAW_DIGIT != 0
}

impl PngStruct {
    /// Port of `png_handle_pCAL` (pngrutil.c#L1219-L1299).
    #[doc(alias = "png_handle_pCAL")]
    pub(crate) fn handle_pcal(
        &mut self,
        _info: &mut PngInfo,
        length: u32,
    ) -> PngResult<HandleResult> {
        let Some(mut buffer) = self.read_buffer_alloc(length + 1) else {
            self.crc_finish(length)?;
            self.chunk_benign_error("out of memory")?;
            return Ok(HandleResult::Error);
        };
        self.crc_read(&mut buffer[..length as usize]);
        if self.crc_finish(0)? {
            return Ok(HandleResult::Error);
        }
        let end = length as usize;
        buffer[end] = 0;
        // The purpose string ends at the first zero byte.
        let mut buf = 0usize;
        while buffer[buf] != 0 {
            buf += 1;
        }
        if end.wrapping_sub(buf) <= 12 {
            self.chunk_benign_error("invalid")?;
            return Ok(HandleResult::Error);
        }
        let _x0 = get_int_32(&buffer[buf + 1..buf + 5]);
        let _x1 = get_int_32(&buffer[buf + 5..buf + 9]);
        let kind = buffer[buf + 9];
        let nparams = buffer[buf + 10];
        buf += 11;
        if (kind == PNG_EQUATION_LINEAR && nparams != 2)
            || (kind == PNG_EQUATION_BASE_E && nparams != 3)
            || (kind == PNG_EQUATION_ARBITRARY && nparams != 3)
            || (kind == PNG_EQUATION_HYPERBOLIC && nparams != 4)
        {
            self.chunk_benign_error("invalid parameter count")?;
            return Ok(HandleResult::Error);
        } else if kind >= PNG_EQUATION_LAST {
            self.chunk_benign_error("unrecognized equation type")?;
        }
        // Skip the units string.
        while buffer[buf] != 0 {
            buf += 1;
        }
        for _ in 0..nparams {
            buf += 1;
            while buf <= end && buffer[buf] != 0 {
                buf += 1;
            }
            if buf > end {
                self.chunk_benign_error("invalid data")?;
                return Ok(HandleResult::Error);
            }
        }
        Ok(HandleResult::Ok)
    }

    /// Port of `png_handle_sCAL` (pngrutil.c#L1301-L1349).
    #[doc(alias = "png_handle_sCAL")]
    pub(crate) fn handle_scal(
        &mut self,
        _info: &mut PngInfo,
        length: u32,
    ) -> PngResult<HandleResult> {
        let Some(mut buffer) = self.read_buffer_alloc(length + 1) else {
            self.crc_finish(length)?;
            self.chunk_benign_error("out of memory")?;
            return Ok(HandleResult::Error);
        };
        self.crc_read(&mut buffer[..length as usize]);
        let end = length as usize;
        buffer[end] = 0;
        if self.crc_finish(0)? {
            return Ok(HandleResult::Error);
        }
        if buffer[0] != 1 && buffer[0] != 2 {
            self.chunk_benign_error("invalid unit")?;
            return Ok(HandleResult::Error);
        }
        let mut i = 1usize;
        let mut state = 0i32;
        let width_ok = check_fp_number(&buffer, end, &mut state, &mut i);
        let width_ok = width_ok && i < end && {
            let c = buffer[i];
            i += 1;
            c == 0
        };
        if !width_ok {
            self.chunk_benign_error("bad width format")?;
        } else if !fp_is_positive(state) {
            self.chunk_benign_error("non-positive width")?;
        } else {
            let heighti = i;
            state = 0;
            let height_ok = check_fp_number(&buffer, end, &mut state, &mut i) && i == end;
            if !height_ok {
                self.chunk_benign_error("bad height format")?;
            } else if !fp_is_positive(state) {
                self.chunk_benign_error("non-positive height")?;
            } else {
                // png_set_sCAL_s: the unit, width and height are stored by libpng. Not kept here.
                let _ = heighti;
                return Ok(HandleResult::Ok);
            }
        }
        Ok(HandleResult::Error)
    }

    /// Port of `png_handle_sPLT` (pngrutil.c#L834-L925). The palette is validated and dropped.
    #[doc(alias = "png_handle_sPLT")]
    pub(crate) fn handle_splt(
        &mut self,
        _info: &mut PngInfo,
        length: u32,
    ) -> PngResult<HandleResult> {
        // The cache countdown of png_handle_sPLT (pngrutil.c#L840-L852). Unlike the text chunks,
        // running out of space here is a warning.
        if self.user_chunk_cache_max != 0 {
            if self.user_chunk_cache_max == 1 {
                self.crc_finish(length)?;
                return Ok(HandleResult::Error);
            }
            self.user_chunk_cache_max -= 1;
            if self.user_chunk_cache_max == 1 {
                self.warning("No space in chunk cache for sPLT");
                self.crc_finish(length)?;
                return Ok(HandleResult::Error);
            }
        }
        let Some(mut buffer) = self.read_buffer_alloc(length + 1) else {
            self.crc_finish(length)?;
            self.chunk_benign_error("out of memory")?;
            return Ok(HandleResult::Error);
        };
        self.crc_read(&mut buffer[..length as usize]);
        if self.crc_finish(0)? {
            return Ok(HandleResult::Error);
        }
        let end = length as usize;
        buffer[end] = 0;
        // entry_start: the byte after the name's terminating zero.
        let mut entry_start = 0usize;
        while buffer[entry_start] != 0 {
            entry_start += 1;
        }
        entry_start += 1;
        if length < 2 || entry_start > end - 2 {
            self.warning("malformed sPLT chunk");
            return Ok(HandleResult::Error);
        }
        let depth = buffer[entry_start];
        entry_start += 1;
        let entry_size: usize = if depth == 8 { 6 } else { 10 };
        let data_length = end - entry_start;
        if !data_length.is_multiple_of(entry_size) {
            self.warning("sPLT chunk has bad length");
            return Ok(HandleResult::Error);
        }
        // png_set_sPLT stores the entries, which are not kept here. Only their layout is checked.
        Ok(HandleResult::Ok)
    }
}

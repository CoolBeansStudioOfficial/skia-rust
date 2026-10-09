// Copyright (C) 1998-2025 Glenn Randers-Pehrson and the libpng contributors.
// Use of this source code is governed by the libpng licence (libpng-2.0) in the LICENSE file.
// Port of: pngrutil.c (libpng 1.6.56, skia.googlesource.com/third_party/libpng@d5515b5b).
// Only the functions the progressive read path reaches are ported here; the sequential reader
// (`png_read_info`, `png_read_IDAT_data`, `png_read_row`) is not ported.

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
use skia_rust_zlib::{Flush, Inflate, ReturnCode};

use crate::error::PngResult;
use crate::structs::{PngColor, PngColor8, PngInfo, PngStruct, flag, mode};

/// Port of `png_handle_result_code` (pngpriv.h#L1788-L1791). The order is significant: `Saved`
/// and above count as the chunk being handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum HandleResult {
    /// Port of `handled_error`: bad CRC, a known chunk in a bad format, or too long.
    Error,
    /// Port of `handled_discarded`: not saved in the unknown chunk list.
    Discard,
    /// Port of `handled_saved`: saved in the unknown chunk list.
    Saved,
    /// Port of `handled_ok`: known, supported and handled without error.
    Ok,
}

/// Port of `PNG_MAX_PALETTE_LENGTH` (png.h).
pub(crate) const PNG_MAX_PALETTE_LENGTH: u32 = 256;

/// Port of `PNG_INFLATE_BUF_SIZE` (pnglibconf.h, Skia's configuration).
pub(crate) const PNG_INFLATE_BUF_SIZE: usize = 1024;

/// Port of `PNG_GAMMA_sRGB_INVERSE` (png.h): 1/2.2 times 100000.
pub(crate) const PNG_GAMMA_SRGB_INVERSE: u32 = 45455;

/// Port of `PNG_UINT_31_MAX` (png.h).
pub(crate) const PNG_UINT_31_MAX: u32 = 0x7fff_ffff;

/// Port of `png_get_uint_32` (pngrutil.c#L24-L30).
#[must_use]
pub(crate) fn get_uint_32(buf: &[u8]) -> u32 {
    (u32::from(buf[0]) << 24)
        + (u32::from(buf[1]) << 16)
        + (u32::from(buf[2]) << 8)
        + u32::from(buf[3])
}

/// Port of `png_get_int_32` (pngrutil.c#L36-L47).
#[must_use]
pub(crate) fn get_int_32(buf: &[u8]) -> i32 {
    let mut uval = get_uint_32(buf);
    if uval & 0x8000_0000 == 0 {
        return uval as i32;
    }
    uval = (uval ^ 0xffff_ffff).wrapping_add(1);
    if uval & 0x8000_0000 == 0 {
        return -(uval as i32);
    }
    0
}

/// Port of `png_get_uint_16` (pngrutil.c#L49-L56).
#[must_use]
pub(crate) fn get_uint_16(buf: &[u8]) -> u16 {
    ((u32::from(buf[0]) << 8) + u32::from(buf[1])) as u16
}

/// Port of `check_chunk_name` (pngrutil.c#L69-L80): the four bytes are ASCII letters.
#[must_use]
fn check_chunk_name(name: u32) -> bool {
    let mut name = name;
    let mut t: u32;
    name &= !0x2020_0020u32;
    t = (name & !0x1f1f_1f1fu32) ^ 0x4040_4040u32;
    name = name.wrapping_sub(0x4141_4141u32);
    t |= name;
    name = name.wrapping_sub(0x1919_191au32);
    t |= !name;
    (t & 0xe0e0_e0e0u32) == 0
}

impl PngStruct {
    /// Port of `png_get_uint_31` (pngrutil.c#L19-L25): a length or dimension, at most 2^31-1.
    #[doc(alias = "png_get_uint_31")]
    pub(crate) fn get_uint_31(&mut self, buf: &[u8]) -> PngResult<u32> {
        let uval = get_uint_32(buf);
        if uval > PNG_UINT_31_MAX {
            return Err(self.error("PNG unsigned integer out of range"));
        }
        Ok(uval)
    }

    /// Port of `png_read_chunk_header` (pngrutil.c#L109-L154) for the progressive reader. Reads the
    /// eight header bytes from the input (the caller checked there are enough).
    #[doc(alias = "png_read_chunk_header")]
    pub(crate) fn read_chunk_header(&mut self) -> PngResult<u32> {
        let mut buf = [0u8; 8];
        self.read_data(&mut buf);
        let length = self.get_uint_31(&buf)?;
        let chunk_name = crate::structs::chunk_from_bytes(&buf[4..8]);
        self.chunk_name = chunk_name;
        self.reset_crc();
        self.calculate_crc(&buf[4..8]);
        if buf[0] >= 0x80 {
            return Err(self.chunk_error("bad header (invalid length)"));
        }
        if !check_chunk_name(chunk_name) {
            return Err(self.chunk_error("bad header (invalid type)"));
        }
        Ok(length)
    }

    /// Port of `png_read_data` (pngrio.c#L23-L33) for the progressive reader: the read function is
    /// `png_push_fill_buffer`.
    pub(crate) fn read_data(&mut self, buf: &mut [u8]) {
        self.push_fill_buffer(buf);
    }

    /// Port of `png_crc_read` (pngrutil.c#L127-L135).
    #[doc(alias = "png_crc_read")]
    pub(crate) fn crc_read(&mut self, buf: &mut [u8]) {
        self.read_data(buf);
        self.calculate_crc(buf);
    }

    /// Port of `png_crc_error` (pngrutil.c#L146-L183). Returns true if the CRC does not match.
    fn crc_error(&mut self, handle_as_ancillary: bool) -> bool {
        let mut need_crc = true;
        if handle_as_ancillary || crate::structs::chunk_ancillary(self.chunk_name) != 0 {
            if self.flags & flag::CRC_ANCILLARY_MASK
                == (flag::CRC_ANCILLARY_USE | flag::CRC_ANCILLARY_NOWARN)
            {
                need_crc = false;
            }
        } else if self.flags & flag::CRC_CRITICAL_IGNORE != 0 {
            need_crc = false;
        }
        let mut crc_bytes = [0u8; 4];
        self.read_data(&mut crc_bytes);
        if need_crc {
            let crc = get_uint_32(&crc_bytes);
            crc != self.crc
        } else {
            false
        }
    }

    /// Port of `png_crc_finish_critical` (pngrutil.c#L186-L223). Returns `Ok(true)` when a CRC
    /// warning was given instead of an error (C's return value 1).
    fn crc_finish_critical(&mut self, skip: u32, handle_as_ancillary: bool) -> PngResult<bool> {
        let mut skip = skip;
        while skip > 0 {
            let len = core::cmp::min(PNG_INFLATE_BUF_SIZE as u32, skip) as usize;
            skip -= len as u32;
            let mut tmpbuf = vec![0u8; len];
            self.crc_read(&mut tmpbuf);
        }
        let mut handle_as_ancillary = handle_as_ancillary;
        if handle_as_ancillary && self.flags & flag::CRC_CRITICAL_IGNORE != 0 {
            handle_as_ancillary = false;
        }
        if self.crc_error(handle_as_ancillary) {
            let warn =
                if handle_as_ancillary || crate::structs::chunk_ancillary(self.chunk_name) != 0 {
                    self.flags & flag::CRC_ANCILLARY_NOWARN == 0
                } else {
                    self.flags & flag::CRC_CRITICAL_USE != 0
                };
            if warn {
                self.chunk_warning("CRC error");
            } else {
                return Err(self.chunk_error("CRC error"));
            }
            return Ok(true);
        }
        Ok(false)
    }

    /// Port of `png_crc_finish` (pngrutil.c#L226-L229).
    #[doc(alias = "png_crc_finish")]
    pub(crate) fn crc_finish(&mut self, skip: u32) -> PngResult<bool> {
        self.crc_finish_critical(skip, false)
    }

    /// Port of `png_inflate_claim` (pngrutil.c#L254-L300). Takes the inflate stream for `owner`.
    pub(crate) fn inflate_claim(&mut self, owner: u32) -> ReturnCode {
        if self.zowner != 0 {
            let msg = format!(
                "{} using zstream",
                crate::structs::chunk_string(self.zowner)
            );
            self.chunk_warning(&msg);
            self.zowner = 0;
        }
        let window_bits: i32;
        if (self.options >> crate::png::PNG_MAXIMUM_INFLATE_WINDOW) & 3 == crate::png::PNG_OPTION_ON
        {
            window_bits = 15;
            self.zstream_start = false;
        } else {
            window_bits = 0;
            self.zstream_start = true;
        }
        self.zstream_avail_out = 0;
        self.zstream_next_out = 0;
        // inflateReset/inflateInit clear zlib's message.
        self.zstream_msg = None;
        match Inflate::new(window_bits) {
            Ok(z) => {
                self.zstream = Some(z);
                self.flags |= flag::ZSTREAM_INITIALIZED;
                self.zowner = owner;
                ReturnCode::Ok
            }
            Err(ret) => {
                self.zstream_error(ret);
                ret
            }
        }
    }

    /// Port of `png_zlib_inflate` (pngrutil.c#L301-L316). Feeds `input` to the inflate stream and
    /// writes into `output`. Returns zlib's return code, how many input bytes were used and how many
    /// output bytes were written.
    pub(crate) fn zlib_inflate(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        flush: Flush,
    ) -> (ReturnCode, usize, usize) {
        if self.zstream_start
            && let Some(&first) = input.first()
        {
            if (first >> 4) > 7 {
                self.zstream_msg = Some("invalid window size (libpng)");
                return (ReturnCode::DataError, 0, 0);
            }
            self.zstream_start = false;
        }
        match self.zstream.as_mut() {
            Some(z) => {
                let r = z.inflate(input, output, flush);
                // zlib keeps its own message in `z_stream.msg`; libpng reports it when it is unset.
                if self.zstream_msg.is_none() {
                    self.zstream_msg = z.msg();
                }
                (r.ret, r.consumed, r.produced)
            }
            None => (ReturnCode::StreamError, 0, 0),
        }
    }

    /// Port of `png_zstream_error` (pngrutil.c#L302-L316 region; png.c). Records zlib's message.
    pub(crate) fn zstream_error(&mut self, ret: ReturnCode) {
        // Port of png_zstream_error (png.c#L545-L580): the message is set only when none is set.
        if self.zstream_msg.is_some() {
            return;
        }
        self.zstream_msg = Some(match ret {
            ReturnCode::Ok => "unexpected zlib return code",
            ReturnCode::StreamEnd => "unexpected end of LZ stream",
            ReturnCode::NeedDict => "missing LZ dictionary",
            ReturnCode::DataError => "LZ data error",
            ReturnCode::StreamError => "LZ stream error",
            ReturnCode::MemError => "zlib memory error",
            ReturnCode::BufError => "zlib buffer error",
        });
    }

    /// Port of `png_handle_IHDR` (pngrutil.c#L440-L490).
    #[doc(alias = "png_handle_IHDR")]
    pub(crate) fn handle_ihdr(
        &mut self,
        info: &mut PngInfo,
        _length: u32,
    ) -> PngResult<HandleResult> {
        self.mode |= mode::HAVE_IHDR;
        let mut buf = [0u8; 13];
        self.crc_read(&mut buf);
        self.crc_finish(0)?;
        let width = self.get_uint_31(&buf[0..4])?;
        let height = self.get_uint_31(&buf[4..8])?;
        let bit_depth = buf[8];
        let color_type = buf[9];
        let compression_type = buf[10];
        let filter_type = buf[11];
        let interlace_type = buf[12];
        self.width = width;
        self.height = height;
        self.bit_depth = bit_depth;
        self.interlaced = interlace_type;
        self.color_type = color_type;
        self.filter_type = filter_type;
        self.compression_type = compression_type;
        self.channels = match self.color_type {
            // Port of the `default:` case (pngrutil.c#L473-L474) that shares `PNG_COLOR_TYPE_GRAY`.
            2 => 3,
            4 => 2,
            6 => 4,
            _ => 1,
        };
        self.pixel_depth = self.bit_depth.wrapping_mul(self.channels);
        self.rowbytes = crate::structs::rowbytes(self.pixel_depth, self.width);
        self.set_ihdr(
            info,
            width,
            height,
            bit_depth,
            color_type,
            interlace_type,
            compression_type,
            filter_type,
        )?;
        Ok(HandleResult::Ok)
    }

    /// Port of `png_handle_PLTE` (pngrutil.c#L492-L542).
    #[doc(alias = "png_handle_PLTE")]
    pub(crate) fn handle_plte(
        &mut self,
        info: &mut PngInfo,
        length: u32,
    ) -> PngResult<HandleResult> {
        let errmsg: Option<&str> = if self.mode & mode::HAVE_PLTE != 0 {
            Some("duplicate")
        } else if self.mode & mode::HAVE_IDAT != 0 {
            Some("out of place")
        } else if self.color_type & 2 == 0 {
            Some("ignored in grayscale PNG")
        } else if length > 3 * PNG_MAX_PALETTE_LENGTH || !length.is_multiple_of(3) {
            Some("invalid")
        } else if self.color_type != 3 && (self.has_chunk("tRNS") || self.has_chunk("bKGD")) {
            Some("out of place")
        } else {
            None
        };
        if errmsg.is_none() {
            let max_palette_length: u32 = if self.color_type == 3 {
                1u32 << self.bit_depth
            } else {
                PNG_MAX_PALETTE_LENGTH
            };
            let num: u32 = if length > 3 * max_palette_length {
                max_palette_length
            } else {
                length / 3
            };
            let mut buf = vec![0u8; (3 * num) as usize];
            self.crc_read(&mut buf);
            self.crc_finish_critical(length - 3 * num, self.color_type != 3)?;
            let mut palette = Vec::with_capacity(num as usize);
            for i in 0..num as usize {
                palette.push(PngColor {
                    red: buf[3 * i],
                    green: buf[3 * i + 1],
                    blue: buf[3 * i + 2],
                });
            }
            self.mode |= mode::HAVE_PLTE;
            self.set_plte(info, &palette, num)?;
            return Ok(HandleResult::Ok);
        }
        let errmsg = errmsg.unwrap_or("");
        if self.color_type == 3 {
            self.crc_finish(length)?;
            return Err(self.chunk_error(errmsg));
        }
        self.crc_finish_critical(length, true)?;
        self.chunk_benign_error(errmsg)?;
        Ok(HandleResult::Error)
    }

    /// Port of `png_handle_IEND` (pngrutil.c#L544-L553).
    #[doc(alias = "png_handle_IEND")]
    pub(crate) fn handle_iend(
        &mut self,
        _info: &mut PngInfo,
        length: u32,
    ) -> PngResult<HandleResult> {
        self.mode |= mode::AFTER_IDAT | mode::HAVE_IEND;
        if length != 0 {
            self.chunk_benign_error("invalid")?;
        }
        self.crc_finish_critical(length, true)?;
        Ok(HandleResult::Ok)
    }

    /// Port of `png_handle_gAMA` (pngrutil.c#L555-L574).
    #[doc(alias = "png_handle_gAMA")]
    pub(crate) fn handle_gama(
        &mut self,
        info: &mut PngInfo,
        _length: u32,
    ) -> PngResult<HandleResult> {
        let mut buf = [0u8; 4];
        self.crc_read(&mut buf);
        if self.crc_finish(0)? {
            return Ok(HandleResult::Error);
        }
        let ugamma = get_uint_32(&buf);
        if ugamma > PNG_UINT_31_MAX {
            self.chunk_benign_error("invalid")?;
            return Ok(HandleResult::Error);
        }
        self.set_gama_fixed(info, ugamma as i32);
        if self.chunk_gamma == 0 {
            self.chunk_gamma = ugamma;
        }
        Ok(HandleResult::Ok)
    }

    /// Port of `png_handle_sBIT` (pngrutil.c#L576-L627).
    #[doc(alias = "png_handle_sBIT")]
    pub(crate) fn handle_sbit(
        &mut self,
        info: &mut PngInfo,
        length: u32,
    ) -> PngResult<HandleResult> {
        let (truelen, sample_depth): (u32, u8) = if self.color_type == 3 {
            (3, 8)
        } else {
            (u32::from(self.channels), self.bit_depth)
        };
        if length != truelen {
            self.crc_finish(length)?;
            self.chunk_benign_error("bad length")?;
            return Ok(HandleResult::Error);
        }
        let mut buf = [sample_depth; 4];
        self.crc_read(&mut buf[..truelen as usize]);
        if self.crc_finish(0)? {
            return Ok(HandleResult::Error);
        }
        for &b in &buf[..truelen as usize] {
            if b == 0 || b > sample_depth {
                self.chunk_benign_error("invalid")?;
                return Ok(HandleResult::Error);
            }
        }
        if self.color_type & 2 != 0 {
            self.sig_bit.red = buf[0];
            self.sig_bit.green = buf[1];
            self.sig_bit.blue = buf[2];
            self.sig_bit.alpha = buf[3];
        } else {
            self.sig_bit.gray = buf[0];
            self.sig_bit.red = buf[0];
            self.sig_bit.green = buf[0];
            self.sig_bit.blue = buf[0];
            self.sig_bit.alpha = buf[1];
        }
        let sb: PngColor8 = self.sig_bit;
        self.set_sbit(info, sb);
        Ok(HandleResult::Ok)
    }

    /// Port of `png_get_int_32_checked` (pngrutil.c#L629-L638). `None` is the `*error = 1` case.
    #[must_use]
    fn get_int_32_checked(buf: &[u8]) -> Option<i32> {
        let mut uval = get_uint_32(buf);
        if uval & 0x8000_0000 == 0 {
            return Some(uval as i32);
        }
        uval = (uval ^ 0xffff_ffff).wrapping_add(1);
        if uval & 0x8000_0000 == 0 {
            return Some(-(uval as i32));
        }
        None
    }

    /// Port of `png_handle_cHRM` (pngrutil.c#L641-L670).
    #[doc(alias = "png_handle_cHRM")]
    pub(crate) fn handle_chrm(
        &mut self,
        info: &mut PngInfo,
        _length: u32,
    ) -> PngResult<HandleResult> {
        let mut buf = [0u8; 32];
        self.crc_read(&mut buf);
        if self.crc_finish(0)? {
            return Ok(HandleResult::Error);
        }
        let mut vals = [0i32; 8];
        let mut error = false;
        for (i, v) in vals.iter_mut().enumerate() {
            match Self::get_int_32_checked(&buf[4 * i..4 * i + 4]) {
                Some(x) => *v = x,
                None => error = true,
            }
        }
        if error {
            self.chunk_benign_error("invalid")?;
            return Ok(HandleResult::Error);
        }
        self.set_chrm_fixed(info, vals);
        if !self.has_chunk("mDCV") {
            self.chromaticities = vals;
        }
        Ok(HandleResult::Ok)
    }

    /// Port of `png_handle_sRGB` (pngrutil.c#L673-L690).
    #[doc(alias = "png_handle_sRGB")]
    pub(crate) fn handle_srgb(
        &mut self,
        info: &mut PngInfo,
        _length: u32,
    ) -> PngResult<HandleResult> {
        let mut intent = [0u8; 1];
        self.crc_read(&mut intent);
        if self.crc_finish(0)? {
            return Ok(HandleResult::Error);
        }
        if intent[0] > 3 {
            self.chunk_benign_error("invalid")?;
            return Ok(HandleResult::Error);
        }
        self.set_srgb(info, intent[0]);
        if !self.has_chunk("cICP") || self.chunk_gamma == 0 {
            self.chunk_gamma = PNG_GAMMA_SRGB_INVERSE;
        }
        Ok(HandleResult::Ok)
    }
}

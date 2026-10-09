// Copyright (C) 1998-2025 Glenn Randers-Pehrson and the libpng contributors.
// Use of this source code is governed by the libpng licence (libpng-2.0) in the LICENSE file.
// Port of: pngrutil.c (libpng 1.6.56, skia.googlesource.com/third_party/libpng@d5515b5b): the
// chunk handlers that decompress data (`png_inflate_read`, `png_handle_iCCP`).

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
use skia_rust_zlib::{Flush, ReturnCode};

use crate::error::PngResult;
use crate::rutil::{HandleResult, PNG_INFLATE_BUF_SIZE, get_uint_32};
use crate::structs::{PngInfo, PngStruct, chunk_from_bytes, flag, info};

/// Port of `LZ77Min` (pngrutil.c#L5): the smallest zlib stream libpng accepts.
const LZ77_MIN: u32 = 2 + 5 + 4;

/// Port of `PNG_COMPRESSION_TYPE_BASE` (png.h): the only compression method.
const PNG_COMPRESSION_TYPE_BASE: u8 = 0;

/// Port of `ZLIB_IO_MAX` (pngpriv.h): the largest count zlib takes in one call. Only affects how
/// many calls are made, not what zlib produces.
const ZLIB_IO_MAX: usize = u32::MAX as usize;

impl PngStruct {
    /// Port of `png_inflate_read` (pngrutil.c#L395-L438). Reads compressed data from the chunk,
    /// `read_size` bytes at a time, and inflates it into `out`. `out_size` is the number of bytes
    /// still wanted on entry and the number not yet written on return, as in C. Unused input stays
    /// in `zin` for the next call, as `zstream.next_in` does in C.
    pub(crate) fn inflate_read(
        &mut self,
        read_size: usize,
        chunk_bytes: &mut u32,
        out: &mut [u8],
        out_size: &mut usize,
        finish: bool,
    ) -> ReturnCode {
        if self.zowner != self.chunk_name {
            self.zstream_msg = Some("zstream unclaimed");
            return ReturnCode::StreamError;
        }
        let mut next_out = 0usize;
        self.zstream_avail_out = 0;
        let mut ret;
        loop {
            if self.zin_pos >= self.zin.len() {
                let mut read_size = read_size;
                if read_size > *chunk_bytes as usize {
                    read_size = *chunk_bytes as usize;
                }
                *chunk_bytes -= read_size as u32;
                let mut tmp = vec![0u8; read_size];
                if read_size > 0 {
                    self.crc_read(&mut tmp);
                }
                self.zin = tmp;
                self.zin_pos = 0;
            }
            if self.zstream_avail_out == 0 {
                let mut avail = ZLIB_IO_MAX;
                if avail > *out_size {
                    avail = *out_size;
                }
                *out_size -= avail;
                self.zstream_avail_out = avail;
            }
            let flush = if *chunk_bytes > 0 {
                Flush::NoFlush
            } else if finish {
                Flush::Finish
            } else {
                Flush::SyncFlush
            };
            let avail_out = self.zstream_avail_out;
            let input = std::mem::take(&mut self.zin);
            let pos = self.zin_pos;
            let (r, consumed, produced) = self.zlib_inflate(
                &input[pos..],
                &mut out[next_out..next_out + avail_out],
                flush,
            );
            self.zin = input;
            self.zin_pos = pos + consumed;
            next_out += produced;
            self.zstream_avail_out -= produced;
            ret = r;
            if !(ret == ReturnCode::Ok && (*out_size > 0 || self.zstream_avail_out > 0)) {
                break;
            }
        }
        *out_size += self.zstream_avail_out;
        self.zstream_avail_out = 0;
        self.zstream_error(ret);
        ret
    }

    /// Port of `png_handle_iCCP` (pngrutil.c#L692-L832).
    #[doc(alias = "png_handle_iCCP")]
    pub(crate) fn handle_iccp(
        &mut self,
        info: &mut PngInfo,
        length: u32,
    ) -> PngResult<HandleResult> {
        let mut length = length;
        let mut read_length: usize = 81;
        if read_length as u32 > length {
            read_length = length as usize;
        }
        let mut keyword = [0u8; 81];
        self.crc_read(&mut keyword[..read_length]);
        length -= read_length as u32;
        if length < LZ77_MIN {
            self.crc_finish(length)?;
            self.chunk_benign_error("too short")?;
            return Ok(HandleResult::Error);
        }
        let mut keyword_length = 0usize;
        while keyword_length < 80 && keyword_length < read_length && keyword[keyword_length] != 0 {
            keyword_length += 1;
        }
        let errmsg: Option<String> = 'err: {
            if !(1..=79).contains(&keyword_length) {
                break 'err Some("bad keyword".into());
            }
            if !(keyword_length + 1 < read_length
                && keyword[keyword_length + 1] == PNG_COMPRESSION_TYPE_BASE)
            {
                break 'err Some("bad compression method".into());
            }
            let read_length = read_length - (keyword_length + 2);
            if self.inflate_claim(chunk_from_bytes(b"iCCP")) != ReturnCode::Ok {
                break 'err self.zstream_msg.map(str::to_owned);
            }
            let outcome =
                self.iccp_inflate_profile(info, &keyword, keyword_length, read_length, &mut length);
            self.zowner = 0;
            match outcome? {
                None => return Ok(HandleResult::Ok),
                Some(msg) => break 'err Some(msg),
            }
        };
        self.crc_finish(length)?;
        if let Some(msg) = errmsg {
            self.chunk_benign_error(&msg)?;
        }
        Ok(HandleResult::Error)
    }

    /// The inflate part of `png_handle_iCCP` (pngrutil.c#L714-L815): the header, the tag table and
    /// the tail of the profile. `Ok(None)` is success (the profile was stored). `Ok(Some)` is the
    /// text the C code would pass to `png_chunk_benign_error`.
    fn iccp_inflate_profile(
        &mut self,
        info: &mut PngInfo,
        keyword_buf: &[u8; 81],
        keyword_length: usize,
        read_length: usize,
        length: &mut u32,
    ) -> PngResult<Option<String>> {
        let keyword = &keyword_buf[..keyword_length];
        // png_ptr->zstream.next_in = keyword + (keyword_length + 2), avail_in = read_length.
        // The compressed bytes were read into the keyword buffer with the keyword.
        let start = keyword_length + 2;
        self.zin = keyword_buf[start..start + read_length].to_vec();
        self.zin_pos = 0;
        let mut profile_header = [0u8; 132];
        let mut size: usize = profile_header.len();
        self.inflate_read(
            PNG_INFLATE_BUF_SIZE,
            length,
            &mut profile_header,
            &mut size,
            false,
        );
        if size != 0 {
            return Ok(Some(self.zstream_msg_text()));
        }
        let profile_length = get_uint_32(&profile_header);
        if !self.icc_check_length(keyword, profile_length)? {
            return Ok(Some(self.zstream_msg_text()));
        }
        if !self.icc_check_header(keyword, profile_length, &profile_header, self.color_type)? {
            return Ok(Some(self.zstream_msg_text()));
        }
        let tag_count = get_uint_32(&profile_header[128..132]);
        let Some(mut profile) = self.read_buffer_alloc(profile_length) else {
            return Ok(Some("out of memory".into()));
        };
        profile[..132].copy_from_slice(&profile_header);
        let mut size = (12 * tag_count) as usize;
        let tag_end = 132 + size;
        self.inflate_read(
            PNG_INFLATE_BUF_SIZE,
            length,
            &mut profile[132..tag_end],
            &mut size,
            false,
        );
        if size != 0 {
            return Ok(Some(self.zstream_msg_text()));
        }
        if !self.icc_check_tag_table(keyword, profile_length, &profile)? {
            return Ok(Some(self.zstream_msg_text()));
        }
        let mut size = profile_length as usize - 132 - 12 * tag_count as usize;
        self.inflate_read(
            PNG_INFLATE_BUF_SIZE,
            length,
            &mut profile[tag_end..],
            &mut size,
            true,
        );
        if *length > 0 && self.flags & flag::BENIGN_ERRORS_WARN == 0 {
            return Ok(Some("extra compressed data".into()));
        }
        if size != 0 {
            return Ok(Some(self.zstream_msg_text()));
        }
        if *length > 0 {
            self.chunk_warning("extra compressed data");
        }
        self.crc_finish(*length)?;
        *length = 0;
        info.iccp_name = String::from_utf8_lossy(keyword).into_owned();
        info.iccp_profile = profile;
        info.iccp_compression = PNG_COMPRESSION_TYPE_BASE;
        info.valid |= info::ICCP;
        Ok(None)
    }

    /// The zlib message, or a generic text when zlib has none (C would pass NULL here).
    fn zstream_msg_text(&self) -> String {
        self.zstream_msg.unwrap_or("unknown zlib error").to_owned()
    }
}

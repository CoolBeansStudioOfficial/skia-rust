// Copyright (C) 1998-2025 Glenn Randers-Pehrson and the libpng contributors.
// Use of this source code is governed by the libpng licence (libpng-2.0) in the LICENSE file.
// Port of: pngpread.c#L1-L945 (libpng 1.6.56, skia.googlesource.com/third_party/libpng@d5515b5b).
//
// The progressive reader: `png_process_data` runs a state machine over the bytes it is given,
// which Skia's codec drives in 4096-byte pieces. Saved partial chunks live in `save_buffer`
// (`PNG_PUSH_SAVE_BUFFER_IF_*` in C); the caller's bytes are `current_buffer`.

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
use crate::filter;
use crate::interlace;
use crate::png::ProgressiveHandler;
use crate::rtran::RowInfo;
use crate::structs::{PngInfo, PngStruct, flag, mode, rowbytes};

/// Port of `PNG_READ_SIG_MODE` and the other `process_mode` values (pngpread.c#L5-L11).
const PNG_READ_SIG_MODE: u8 = 0;
const PNG_READ_CHUNK_MODE: u8 = 1;
const PNG_READ_IDAT_MODE: u8 = 2;
const PNG_READ_DONE_MODE: u8 = 6;

/// Port of `png_pass_start`, `png_pass_inc`, `png_pass_ystart`, `png_pass_yinc` (pngpread.c).
const PNG_PASS_START: [u32; 7] = [0, 4, 0, 2, 0, 1, 0];
const PNG_PASS_INC: [u32; 7] = [8, 8, 4, 4, 2, 2, 1];
const PNG_PASS_YSTART: [u32; 7] = [0, 0, 4, 0, 2, 0, 1];
const PNG_PASS_YINC: [u32; 7] = [8, 8, 8, 4, 4, 2, 2];

/// Port of `PNG_FILTER_VALUE_*` (pngpriv.h).
const PNG_FILTER_VALUE_NONE: u8 = 0;
const PNG_FILTER_VALUE_LAST: u8 = 5;

/// Port of `PNG_INTERLACE` (pngpriv.h, the transform bit).
pub(crate) const PNG_INTERLACE: u32 = 0x0002;

/// Port of `png_sig_cmp` (png.c#L20-L30): compares `num_to_check` signature bytes starting at
/// `start`. Returns 0 on a match, like `memcmp`.
#[must_use]
pub(crate) fn png_sig_cmp(sig: &[u8], start: usize, num_to_check: usize) -> i32 {
    const PNG_SIG: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
    let mut num_to_check = num_to_check;
    if num_to_check > 8 {
        num_to_check = 8;
    } else if num_to_check < 1 {
        return -1;
    }
    if start > 7 {
        return -1;
    }
    if start + num_to_check > 8 {
        num_to_check = 8 - start;
    }
    for i in 0..num_to_check {
        let a = sig[start + i];
        let b = PNG_SIG[start + i];
        if a != b {
            return i32::from(a) - i32::from(b);
        }
    }
    0
}

impl PngStruct {
    /// Port of `png_process_data` (pngpread.c#L36-L47). The caller's bytes are consumed as far as
    /// the state machine can take them. Bytes it cannot use yet are saved for the next call.
    #[doc(alias = "png_process_data")]
    pub fn process_data(&mut self, info: &mut PngInfo, buffer: &[u8]) -> PngResult<()> {
        self.push_restore_buffer(buffer);
        while self.buffer_size != 0 {
            self.process_some_data(info)?;
        }
        Ok(())
    }

    /// Port of `png_process_some_data` (pngpread.c#L101-L123).
    #[doc(alias = "png_process_some_data")]
    pub(crate) fn process_some_data(&mut self, info: &mut PngInfo) -> PngResult<()> {
        match self.process_mode {
            PNG_READ_SIG_MODE => self.push_read_sig(info),
            PNG_READ_CHUNK_MODE => self.push_read_chunk(info),
            PNG_READ_IDAT_MODE => self.push_read_idat(),
            _ => {
                self.buffer_size = 0;
                Ok(())
            }
        }
    }

    /// Port of `png_push_read_sig` (pngpread.c#L125-L154).
    fn push_read_sig(&mut self, info: &mut PngInfo) -> PngResult<()> {
        let num_checked = self.sig_bytes;
        let mut num_to_check = 8 - num_checked;
        if self.buffer_size < num_to_check {
            num_to_check = self.buffer_size;
        }
        self.push_fill_buffer(&mut info.signature[num_checked..num_checked + num_to_check]);
        self.sig_bytes += num_to_check;
        if png_sig_cmp(&info.signature, num_checked, num_to_check) != 0 {
            // `num_to_check - 4` wraps like the size_t arithmetic in C.
            if num_checked < 4
                && png_sig_cmp(&info.signature, num_checked, num_to_check.wrapping_sub(4)) != 0
            {
                return Err(self.error("Not a PNG file"));
            }
            return Err(self.error("PNG file corrupted by ASCII conversion"));
        }
        if self.sig_bytes >= 8 {
            self.process_mode = PNG_READ_CHUNK_MODE;
        }
        Ok(())
    }

    /// Port of `png_push_read_chunk` (pngpread.c#L157-L240).
    fn push_read_chunk(&mut self, info: &mut PngInfo) -> PngResult<()> {
        if self.mode & mode::HAVE_CHUNK_HEADER == 0 {
            if self.buffer_size < 8 {
                self.push_save_buffer();
                return Ok(());
            }
            self.push_length = self.read_chunk_header()?;
            self.mode |= mode::HAVE_CHUNK_HEADER;
        }
        let chunk_name = self.chunk_name;
        if chunk_name == crate::structs::PNG_IDAT {
            if self.mode & mode::AFTER_IDAT != 0 {
                self.mode |= mode::HAVE_CHUNK_AFTER_IDAT;
            }
            if self.mode & mode::HAVE_IHDR == 0 {
                return Err(self.error("Missing IHDR before IDAT"));
            } else if self.color_type == 3 && self.mode & mode::HAVE_PLTE == 0 {
                return Err(self.error("Missing PLTE before IDAT"));
            }
            self.process_mode = PNG_READ_IDAT_MODE;
            if self.mode & mode::HAVE_IDAT != 0
                && self.mode & mode::HAVE_CHUNK_AFTER_IDAT == 0
                && self.push_length == 0
            {
                return Ok(());
            }
            self.mode |= mode::HAVE_IDAT;
            if self.mode & mode::AFTER_IDAT != 0 {
                self.benign_error("Too many IDATs found")?;
            }
        } else if self.mode & mode::HAVE_IDAT != 0 {
            self.mode |= mode::HAVE_CHUNK_AFTER_IDAT | mode::AFTER_IDAT;
        }

        if chunk_name == crate::structs::PNG_IHDR {
            if self.push_length != 13 {
                return Err(self.error("Invalid IHDR length"));
            }
            if self.push_length as usize + 4 > self.buffer_size {
                self.push_save_buffer();
                return Ok(());
            }
            self.handle_chunk(info, self.push_length)?;
        } else if chunk_name == crate::structs::PNG_IEND {
            if self.push_length as usize + 4 > self.buffer_size {
                self.push_save_buffer();
                return Ok(());
            }
            self.handle_chunk(info, self.push_length)?;
            self.process_mode = PNG_READ_DONE_MODE;
            self.push_have_end(info)?;
        } else if self.chunk_unknown_handling_for_name(chunk_name) != 0 {
            let keep = self.chunk_unknown_handling_for_name(chunk_name);
            if self.push_length as usize + 4 > self.buffer_size {
                self.push_save_buffer();
                return Ok(());
            }
            self.handle_unknown(info, self.push_length, keep)?;
            if chunk_name == crate::structs::PNG_PLTE {
                self.mode |= mode::HAVE_PLTE;
            }
        } else if chunk_name == crate::structs::PNG_IDAT {
            self.idat_size = self.push_length;
            self.process_mode = PNG_READ_IDAT_MODE;
            self.push_have_info(info)?;
            self.zstream_avail_out = rowbytes(self.pixel_depth, self.iwidth) + 1;
            self.zstream_next_out = 0;
            return Ok(());
        } else {
            if self.push_length as usize + 4 > self.buffer_size {
                self.push_save_buffer();
                return Ok(());
            }
            self.handle_chunk(info, self.push_length)?;
        }
        self.mode &= !mode::HAVE_CHUNK_HEADER;
        Ok(())
    }

    /// Port of `png_push_fill_buffer` (pngpread.c#L257-L291). Copies `buffer.len()` bytes from the
    /// saved bytes first, then from the current input. This is the `read_data_fn` of the
    /// progressive reader.
    #[doc(alias = "png_push_fill_buffer")]
    pub(crate) fn push_fill_buffer(&mut self, buffer: &mut [u8]) {
        let mut length = buffer.len();
        let mut ptr = 0usize;
        if self.save_buffer_size != 0 {
            let save_size = if length < self.save_buffer_size {
                length
            } else {
                self.save_buffer_size
            };
            let src = self.save_buffer_ptr;
            buffer[ptr..ptr + save_size].copy_from_slice(&self.save_buffer[src..src + save_size]);
            length -= save_size;
            ptr += save_size;
            self.buffer_size -= save_size;
            self.save_buffer_size -= save_size;
            self.save_buffer_ptr += save_size;
        }
        if length != 0 && self.current_buffer_size != 0 {
            let save_size = if length < self.current_buffer_size {
                length
            } else {
                self.current_buffer_size
            };
            let src = self.current_buffer_ptr;
            buffer[ptr..ptr + save_size]
                .copy_from_slice(&self.current_buffer[src..src + save_size]);
            self.buffer_size -= save_size;
            self.current_buffer_size -= save_size;
            self.current_buffer_ptr += save_size;
        }
    }

    /// Port of `png_push_save_buffer` (pngpread.c#L294-L339). Moves the unused bytes of the input
    /// into the save buffer so the next call sees them.
    #[doc(alias = "png_push_save_buffer")]
    pub(crate) fn push_save_buffer(&mut self) {
        // The C code moves the saved bytes to the front, then appends the current input. Both
        // steps together leave the saved bytes followed by the unused input.
        let saved_start = self.save_buffer_ptr;
        let saved_len = self.save_buffer_size;
        let mut new_save: Vec<u8> = Vec::with_capacity(saved_len + self.current_buffer_size);
        new_save.extend_from_slice(&self.save_buffer[saved_start..saved_start + saved_len]);
        if self.current_buffer_size != 0 {
            let s = self.current_buffer_ptr;
            new_save.extend_from_slice(&self.current_buffer[s..s + self.current_buffer_size]);
            self.save_buffer_size += self.current_buffer_size;
            self.current_buffer_size = 0;
        }
        self.save_buffer = new_save;
        self.save_buffer_ptr = 0;
        self.buffer_size = 0;
    }

    /// Port of `png_push_restore_buffer` (pngpread.c#L342-L352).
    fn push_restore_buffer(&mut self, buffer: &[u8]) {
        self.current_buffer = buffer.to_vec();
        self.current_buffer_size = buffer.len();
        self.buffer_size = buffer.len() + self.save_buffer_size;
        self.current_buffer_ptr = 0;
    }

    /// Port of `png_push_read_IDAT` (pngpread.c#L355-L415).
    fn push_read_idat(&mut self) -> PngResult<()> {
        if self.mode & mode::HAVE_CHUNK_HEADER == 0 {
            if self.buffer_size < 8 {
                self.push_save_buffer();
                return Ok(());
            }
            let mut chunk_length = [0u8; 4];
            let mut chunk_tag = [0u8; 4];
            self.push_fill_buffer(&mut chunk_length);
            self.push_length = self.get_uint_31(&chunk_length)?;
            self.reset_crc();
            self.crc_read(&mut chunk_tag);
            self.chunk_name = crate::structs::chunk_from_bytes(&chunk_tag);
            self.mode |= mode::HAVE_CHUNK_HEADER;
            if self.chunk_name != crate::structs::PNG_IDAT {
                self.process_mode = PNG_READ_CHUNK_MODE;
                if self.flags & flag::ZSTREAM_ENDED == 0 {
                    return Err(self.error("Not enough compressed data"));
                }
                return Ok(());
            }
            self.idat_size = self.push_length;
        }

        if self.idat_size != 0 && self.save_buffer_size != 0 {
            let mut save_size = self.save_buffer_size;
            let mut idat_size = self.idat_size as usize;
            if idat_size < save_size {
                save_size = idat_size;
            } else {
                idat_size = save_size;
            }
            let src = self.save_buffer_ptr;
            let bytes = self.save_buffer[src..src + save_size].to_vec();
            self.calculate_crc(&bytes);
            self.process_idat_data(&bytes)?;
            self.idat_size -= idat_size as u32;
            self.buffer_size -= save_size;
            self.save_buffer_size -= save_size;
            self.save_buffer_ptr += save_size;
        }

        if self.idat_size != 0 && self.current_buffer_size != 0 {
            let mut save_size = self.current_buffer_size;
            let mut idat_size = self.idat_size as usize;
            if idat_size < save_size {
                save_size = idat_size;
            } else {
                idat_size = save_size;
            }
            let src = self.current_buffer_ptr;
            let bytes = self.current_buffer[src..src + save_size].to_vec();
            self.calculate_crc(&bytes);
            self.process_idat_data(&bytes)?;
            self.idat_size -= idat_size as u32;
            self.buffer_size -= save_size;
            self.current_buffer_size -= save_size;
            self.current_buffer_ptr += save_size;
        }

        if self.idat_size == 0 {
            if self.buffer_size < 4 {
                self.push_save_buffer();
                return Ok(());
            }
            // The `int` result (1 for a CRC warning) is ignored, as in `png_push_read_IDAT`.
            let _ = self.crc_finish(0)?;
            self.mode &= !mode::HAVE_CHUNK_HEADER;
            self.mode |= mode::AFTER_IDAT;
            self.zowner = 0;
        }
        Ok(())
    }

    /// Port of `png_process_IDAT_data` (pngpread.c#L438-L508): inflates one piece of IDAT data and
    /// emits each full row it completes.
    #[doc(alias = "png_process_IDAT_data")]
    pub(crate) fn process_idat_data(&mut self, buffer: &[u8]) -> PngResult<()> {
        if buffer.is_empty() {
            return Err(self.error("No IDAT data (internal error)"));
        }
        let mut next_in = 0usize;
        let mut avail_in = buffer.len();
        while avail_in > 0 && self.flags & flag::ZSTREAM_ENDED == 0 {
            if self.zstream_avail_out == 0 {
                self.zstream_avail_out = rowbytes(self.pixel_depth, self.iwidth) + 1;
                self.zstream_next_out = 0;
            }
            let out_start = self.zstream_next_out;
            let out_len = self.zstream_avail_out;
            let mut out = std::mem::take(&mut self.row_buf);
            let (ret, consumed, produced) = self.zlib_inflate(
                &buffer[next_in..next_in + avail_in],
                &mut out[out_start..out_start + out_len],
                Flush::SyncFlush,
            );
            self.row_buf = out;
            next_in += consumed;
            avail_in -= consumed;
            self.zstream_avail_out -= produced;
            self.zstream_next_out += produced;

            if ret != ReturnCode::Ok && ret != ReturnCode::StreamEnd {
                self.flags |= flag::ZSTREAM_ENDED;
                self.zowner = 0;
                if self.row_number >= self.num_rows || self.pass > 6 {
                    self.warning("Truncated compressed data in IDAT");
                } else if ret == ReturnCode::DataError {
                    self.benign_error("IDAT: ADLER32 checksum mismatch")?;
                } else {
                    return Err(self.error("Decompression error in IDAT"));
                }
                return Ok(());
            }
            if self.zstream_next_out != 0 {
                if self.row_number >= self.num_rows || self.pass > 6 {
                    self.warning("Extra compressed data in IDAT");
                    self.flags |= flag::ZSTREAM_ENDED;
                    self.zowner = 0;
                    return Ok(());
                }
                if self.zstream_avail_out == 0 {
                    self.push_process_row()?;
                }
            }
            if ret == ReturnCode::StreamEnd {
                self.flags |= flag::ZSTREAM_ENDED;
            }
        }
        if avail_in > 0 {
            self.warning("Extra compression data in IDAT");
        }
        Ok(())
    }

    /// Port of `png_push_process_row` (pngpread.c#L511-L611).
    fn push_process_row(&mut self) -> PngResult<()> {
        // Port of the `row_info` local of png_push_process_row (pngpread.c#L515-L521).
        let mut row_info = RowInfo {
            width: self.iwidth,
            rowbytes: rowbytes(self.pixel_depth, self.iwidth),
            color_type: self.color_type,
            bit_depth: self.bit_depth,
            channels: self.channels,
            pixel_depth: self.pixel_depth,
        };
        let rb = row_info.rowbytes;
        let filter = self.row_buf[0];
        if filter > PNG_FILTER_VALUE_NONE {
            if filter < PNG_FILTER_VALUE_LAST {
                let pixel_depth = self.pixel_depth;
                filter::read_filter_row(
                    &mut self.row_buf[1..=rb],
                    &self.prev_row[1..=rb],
                    rb,
                    pixel_depth,
                    filter,
                );
            } else {
                return Err(self.error("bad adaptive filter value"));
            }
        }
        let n = rb + 1;
        let (row_buf, prev_row) = (&self.row_buf[..n], &mut self.prev_row[..n]);
        prev_row.copy_from_slice(row_buf);
        if self.transformations != 0 {
            let mut row = std::mem::take(&mut self.row_buf);
            self.do_read_transformations(&mut row_info, &mut row[1..]);
            self.row_buf = row;
        }
        if self.transformed_pixel_depth == 0 {
            self.transformed_pixel_depth = row_info.pixel_depth;
            if row_info.pixel_depth > self.maximum_pixel_depth {
                return Err(self.error("progressive row overflow"));
            }
        } else if self.transformed_pixel_depth != row_info.pixel_depth {
            return Err(self.error("internal progressive row size calculation error"));
        }
        if self.interlaced != 0 && self.transformations & PNG_INTERLACE != 0 {
            if self.pass < 6 {
                // The expanded width is only needed by the pass rows that follow, which use
                // `iwidth` and the pass tables, so the updated row_info is not kept.
                let mut width = row_info.width;
                let mut rb_out = row_info.rowbytes;
                let transformations = self.transformations;
                let pass = self.pass;
                interlace::do_read_interlace(
                    &mut self.row_buf[1..],
                    &mut width,
                    &mut rb_out,
                    row_info.pixel_depth,
                    pass,
                    transformations,
                );
            }
            self.push_rows_for_pass()?;
        } else {
            self.push_have_row_row_buf()?;
            self.read_push_finish_row();
        }
        Ok(())
    }

    /// Port of the `switch (png_ptr->pass)` in `png_push_process_row` (pngpread.c#L560-L659). Each
    /// pass emits its rows, with NULL rows for the gaps that Adam7 interleaves.
    fn push_rows_for_pass(&mut self) -> PngResult<()> {
        match self.pass {
            0 => {
                for _ in 0..8 {
                    if self.pass != 0 {
                        break;
                    }
                    self.push_have_row_row_buf()?;
                    self.read_push_finish_row();
                }
                if self.pass == 2 {
                    for _ in 0..4 {
                        if self.pass != 2 {
                            break;
                        }
                        self.push_have_row(None)?;
                        self.read_push_finish_row();
                    }
                }
                if self.pass == 4 && self.height <= 4 {
                    for _ in 0..2 {
                        if self.pass != 4 {
                            break;
                        }
                        self.push_have_row(None)?;
                        self.read_push_finish_row();
                    }
                }
                if self.pass == 6 && self.height <= 4 {
                    self.push_have_row(None)?;
                    self.read_push_finish_row();
                }
            }
            1 => {
                for _ in 0..8 {
                    if self.pass != 1 {
                        break;
                    }
                    self.push_have_row_row_buf()?;
                    self.read_push_finish_row();
                }
                if self.pass == 2 {
                    for _ in 0..4 {
                        if self.pass != 2 {
                            break;
                        }
                        self.push_have_row(None)?;
                        self.read_push_finish_row();
                    }
                }
            }
            2 => {
                for _ in 0..4 {
                    if self.pass != 2 {
                        break;
                    }
                    self.push_have_row_row_buf()?;
                    self.read_push_finish_row();
                }
                for _ in 0..4 {
                    if self.pass != 2 {
                        break;
                    }
                    self.push_have_row(None)?;
                    self.read_push_finish_row();
                }
                if self.pass == 4 {
                    for _ in 0..2 {
                        if self.pass != 4 {
                            break;
                        }
                        self.push_have_row(None)?;
                        self.read_push_finish_row();
                    }
                }
            }
            3 => {
                for _ in 0..4 {
                    if self.pass != 3 {
                        break;
                    }
                    self.push_have_row_row_buf()?;
                    self.read_push_finish_row();
                }
                if self.pass == 4 {
                    for _ in 0..2 {
                        if self.pass != 4 {
                            break;
                        }
                        self.push_have_row(None)?;
                        self.read_push_finish_row();
                    }
                }
            }
            4 => {
                for _ in 0..2 {
                    if self.pass != 4 {
                        break;
                    }
                    self.push_have_row_row_buf()?;
                    self.read_push_finish_row();
                }
                for _ in 0..2 {
                    if self.pass != 4 {
                        break;
                    }
                    self.push_have_row(None)?;
                    self.read_push_finish_row();
                }
                if self.pass == 6 {
                    self.push_have_row(None)?;
                    self.read_push_finish_row();
                }
            }
            5 => {
                for _ in 0..2 {
                    if self.pass != 5 {
                        break;
                    }
                    self.push_have_row_row_buf()?;
                    self.read_push_finish_row();
                }
                if self.pass == 6 {
                    self.push_have_row(None)?;
                    self.read_push_finish_row();
                }
            }
            _ => {
                // `default:` falls through into `case 6` in C.
                self.push_have_row_row_buf()?;
                self.read_push_finish_row();
                if self.pass == 6 {
                    self.push_have_row(None)?;
                    self.read_push_finish_row();
                }
            }
        }
        Ok(())
    }

    /// Port of `png_read_push_finish_row` (pngpread.c#L614-L656).
    pub(crate) fn read_push_finish_row(&mut self) {
        self.row_number += 1;
        if self.row_number < self.num_rows {
            return;
        }
        if self.interlaced != 0 {
            self.row_number = 0;
            let n = self.rowbytes + 1;
            self.prev_row[..n].fill(0);
            loop {
                self.pass += 1;
                if (self.pass == 1 && self.width < 5)
                    || (self.pass == 3 && self.width < 3)
                    || (self.pass == 5 && self.width < 2)
                {
                    self.pass += 1;
                }
                if self.pass > 7 {
                    self.pass -= 1;
                }
                if self.pass >= 7 {
                    break;
                }
                let p = self.pass as usize;
                self.iwidth =
                    (self.width + PNG_PASS_INC[p] - 1 - PNG_PASS_START[p]) / PNG_PASS_INC[p];
                if self.transformations & PNG_INTERLACE != 0 {
                    break;
                }
                self.num_rows =
                    (self.height + PNG_PASS_YINC[p] - 1 - PNG_PASS_YSTART[p]) / PNG_PASS_YINC[p];
                if !(self.iwidth == 0 || self.num_rows == 0) {
                    break;
                }
            }
        }
    }

    /// Port of `png_push_have_info` (pngpread.c#L659-L665).
    fn push_have_info(&mut self, info: &mut PngInfo) -> PngResult<()> {
        self.with_progressive(|h, png| h.info(png, info))
    }

    /// Port of `png_push_have_end` (pngpread.c#L668-L674).
    fn push_have_end(&mut self, info: &mut PngInfo) -> PngResult<()> {
        self.with_progressive(|h, png| h.end(png, info))
    }

    /// Port of `png_push_have_row` (pngpread.c#L677-L684). `row` is `None` for the empty rows of
    /// an interlaced pass.
    fn push_have_row(&mut self, row: Option<()>) -> PngResult<()> {
        let row_num = self.row_number;
        let pass = self.pass as i32;
        let data: Option<Vec<u8>> = row.map(|()| self.row_buf[1..].to_vec());
        self.with_progressive(|h, png| h.row(png, data.as_deref(), row_num, pass))
    }

    /// Port of `png_push_have_row(png_ptr, png_ptr->row_buf + 1)`.
    fn push_have_row_row_buf(&mut self) -> PngResult<()> {
        self.push_have_row(Some(()))
    }

    /// Calls `f` with the progressive handler, if one is set. The handler is taken out of `self`
    /// for the call so that it can borrow the reader, and put back afterwards even on error.
    fn with_progressive<F>(&mut self, f: F) -> PngResult<()>
    where
        F: FnOnce(&mut dyn ProgressiveHandler, &mut PngStruct) -> PngResult<()>,
    {
        if let Some(mut handler) = self.progressive.take() {
            let result = f(handler.as_mut(), self);
            // A callback that installed a new handler keeps it, as `png_set_progressive_read_fn`
            // does when called from a callback.
            if self.progressive.is_none() {
                self.progressive = Some(handler);
            }
            result
        } else {
            Ok(())
        }
    }

    /// Port of `png_progressive_combine_row` (pngpread.c#L688-L697) and `png_combine_row`
    /// (pngrutil.c). Called by Skia's interlaced row callback.
    #[doc(alias = "png_progressive_combine_row")]
    pub fn progressive_combine_row(
        &mut self,
        old_row: &mut [u8],
        new_row: Option<&[u8]>,
    ) -> PngResult<()> {
        if new_row.is_some() {
            // png_combine_row(png_ptr, old_row, 1) reads the current pass row from row_buf.
            self.combine_row(old_row, 1)?;
        }
        Ok(())
    }

    /// Port of `png_set_progressive_read_fn` (pngpread.c#L703-L717).
    #[doc(alias = "png_set_progressive_read_fn")]
    pub fn set_progressive_read_fn(&mut self, handler: Option<Box<dyn ProgressiveHandler>>) {
        self.progressive = handler;
    }
}

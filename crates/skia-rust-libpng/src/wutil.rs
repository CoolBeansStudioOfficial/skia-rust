// Copyright (C) 1998-2026 Glenn Randers-Pehrson and the libpng contributors.
// Copyright (C) 2026 The skia-rust Authors.
// Use of this source code is governed by the libpng licence (libpng-2.0) in the LICENSE file.
// Port of: pngwutil.c (libpng 1.6.56, skia.googlesource.com/third_party/libpng@d5515b5b): the
// chunk writer, the IHDR, PLTE, sRGB, iCCP, sBIT, tEXt and zTXt writers, the IDAT compressor
// with its deflate claim, and the row filters. Interlaced rows are not ported (Skia writes
// `PNG_INTERLACE_NONE` only).

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
    clippy::cognitive_complexity,
    clippy::many_single_char_names,
    clippy::similar_names
)]

use skia_rust_zlib::{Deflate, Flush, ReturnCode};

use crate::error::PngResult;
use crate::structs::{
    PNG_IDAT, PNG_IHDR, PNG_PLTE, PngColor, PngColor8, PngStruct, chunk_string, flag, mode,
};
use crate::write::{PNG_ALL_FILTERS, PNG_NO_FILTERS, PNG_Z_DEFAULT_STRATEGY};

/// Port of `PNG_FILTER_NONE` .. `PNG_FILTER_PAETH` (png.h).
pub(crate) const PNG_FILTER_NONE: u8 = 0x08;
pub(crate) const PNG_FILTER_SUB: u8 = 0x10;
pub(crate) const PNG_FILTER_UP: u8 = 0x20;
pub(crate) const PNG_FILTER_AVG: u8 = 0x40;
pub(crate) const PNG_FILTER_PAETH: u8 = 0x80;

/// Port of `PNG_FILTER_VALUE_*` (png.h): the filter type byte written at the start of a row.
const PNG_FILTER_VALUE_SUB: u8 = 1;
const PNG_FILTER_VALUE_UP: u8 = 2;
const PNG_FILTER_VALUE_AVG: u8 = 3;
const PNG_FILTER_VALUE_PAETH: u8 = 4;

/// Chunk names the write path emits (pngpriv.h `png_sRGB` etc.).
pub(crate) const PNG_SRGB: u32 = u32::from_be_bytes(*b"sRGB");
pub(crate) const PNG_ICCP: u32 = u32::from_be_bytes(*b"iCCP");
pub(crate) const PNG_SBIT: u32 = u32::from_be_bytes(*b"sBIT");
pub(crate) const PNG_TEXT: u32 = u32::from_be_bytes(*b"tEXt");
pub(crate) const PNG_ZTXT: u32 = u32::from_be_bytes(*b"zTXt");

/// Port of `PNG_UINT_31_MAX` (png.h).
const PNG_UINT_31_MAX: usize = 0x7fff_ffff;
/// Port of `PNG_KEYWORD_MAX_LENGTH` (png.h).
const PNG_KEYWORD_MAX_LENGTH: usize = 79;
/// Port of `PNG_MAX_PALETTE_LENGTH` (pngpriv.h).
const PNG_MAX_PALETTE_LENGTH: usize = 256;
/// Port of `ZLIB_IO_MAX` (pngstruct.h): `(uInt)-1`, so one call takes all the input it can.
const ZLIB_IO_MAX: usize = u32::MAX as usize;

/// Port of the `PNG_COLOR_MASK_*` bits (png.h).
const PNG_COLOR_MASK_COLOR: u8 = 2;
const PNG_COLOR_MASK_ALPHA: u8 = 4;
/// Port of `PNG_COLOR_TYPE_PALETTE` (png.h).
const PNG_COLOR_TYPE_PALETTE: u8 = 3;
/// Port of `PNG_sRGB_INTENT_LAST` (png.h).
const PNG_SRGB_INTENT_LAST: i32 = 4;
/// Port of `PNG_COMPRESSION_TYPE_BASE` (png.h).
const PNG_COMPRESSION_TYPE_BASE: u8 = 0;

/// Port of the `sum += (v < 128) ? v : 256 - v` term of the filter heuristic (pngwutil.c).
#[inline]
fn abs_sum_term(v: u8) -> usize {
    // Port of the `PNG_USE_ABS`-undefined branch: `(v < 128) ? v : 256 - v`.
    let v = usize::from(v);
    if v < 128 { v } else { 256 - v }
}

/// Port of `png_save_uint_32` (pngwutil.c#L41-L49), returned as the four bytes.
#[inline]
pub(crate) fn save_uint_32(i: u32) -> [u8; 4] {
    i.to_be_bytes()
}

/// Port of `png_get_uint_32` (png.c): reads a big-endian u32.
#[inline]
pub(crate) fn get_uint_32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

/// Port of `png_check_keyword` (pngset.c#L1933-L2004). Returns the keyword as stored (with its
/// terminating zero, which is the last byte of the vector) and its length, or length zero for
/// an empty keyword. Warnings are dropped, as they do not change the result.
pub(crate) fn check_keyword(key: &[u8]) -> (usize, Vec<u8>) {
    // The C string ends at the first zero byte.
    let key = match key.iter().position(|&b| b == 0) {
        Some(end) => &key[..end],
        None => key,
    };
    let mut new_key: Vec<u8> = Vec::new();
    let mut key_len: usize = 0;
    let mut bad_character: u8 = 0;
    let mut space = true;
    let mut i = 0;
    while i < key.len() && key_len < PNG_KEYWORD_MAX_LENGTH {
        let ch = key[i];
        i += 1;
        if (ch > 32 && ch <= 126) || ch >= 161 {
            new_key.push(ch);
            key_len += 1;
            space = false;
        } else if !space {
            // A space or an invalid character when one wasn't seen immediately before: output
            // just a space.
            new_key.push(32);
            key_len += 1;
            space = true;
            if ch != 32 {
                bad_character = ch;
            }
        } else if bad_character == 0 {
            bad_character = ch;
        }
    }
    if key_len > 0 && space {
        // Trailing space.
        key_len -= 1;
        new_key.pop();
        if bad_character == 0 {
            bad_character = 32;
        }
    }
    let _ = bad_character;
    if key_len == 0 {
        return (0, vec![0]);
    }
    new_key.push(0);
    (key_len, new_key)
}

/// Port of `optimize_cmf` (pngwutil.c#L267-L302): shrinks the window size in the zlib header of
/// a stream whose uncompressed size is `data_size`, when the data fits in a smaller window.
pub(crate) fn optimize_cmf(data: &mut [u8], data_size: usize) {
    // Optimize the CMF field in the zlib stream. The resultant zlib stream is still compliant
    // to the stream specification.
    if data_size <= 16384 {
        // else windowBits must be 15
        let mut z_cmf = u32::from(data[0]);
        if (z_cmf & 0x0f) == 8 && (z_cmf & 0xf0) <= 0x70 {
            let mut z_cinfo = z_cmf >> 4;
            let mut half_z_window_size: u32 = 1u32 << (z_cinfo + 7);
            if data_size <= half_z_window_size as usize {
                // else no change
                loop {
                    half_z_window_size >>= 1;
                    z_cinfo = z_cinfo.wrapping_sub(1);
                    if !(z_cinfo > 0 && data_size <= half_z_window_size as usize) {
                        break;
                    }
                }
                z_cmf = (z_cmf & 0x0f) | (z_cinfo << 4);
                data[0] = z_cmf as u8;
                let mut tmp = u32::from(data[1]) & 0xe0;
                tmp = tmp.wrapping_add(0x1f - ((z_cmf << 8) + tmp) % 0x1f);
                data[1] = tmp as u8;
            }
        }
    }
}

impl PngStruct {
    /// Port of `png_write_chunk_header` (pngwutil.c#L90-L127).
    fn write_chunk_header(&mut self, chunk_name: u32, length: u32) -> PngResult<()> {
        let mut buf = [0u8; 8];
        buf[..4].copy_from_slice(&save_uint_32(length));
        buf[4..].copy_from_slice(&save_uint_32(chunk_name));
        self.write_data(&buf)?;
        // Put the chunk name into png_ptr->chunk_name, reset the CRC and run it over the name.
        self.chunk_name = chunk_name;
        self.reset_crc();
        self.calculate_crc(&buf[4..]);
        Ok(())
    }

    /// Port of `png_write_chunk_data` (pngwutil.c#L144-L160).
    fn write_chunk_data(&mut self, data: &[u8]) -> PngResult<()> {
        if !data.is_empty() {
            self.write_data(data)?;
            // Update the CRC after writing the data, in case the user I/O routine alters it.
            self.calculate_crc(data);
        }
        Ok(())
    }

    /// Port of `png_write_chunk_end` (pngwutil.c#L163-L174).
    fn write_chunk_end(&mut self) -> PngResult<()> {
        let buf = save_uint_32(self.crc);
        self.write_data(&buf)
    }

    /// Port of `png_write_complete_chunk` (pngwutil.c#L192-L206).
    pub(crate) fn write_complete_chunk(&mut self, chunk_name: u32, data: &[u8]) -> PngResult<()> {
        // On 64-bit architectures 'length' may not fit in a png_uint_32.
        if data.len() > PNG_UINT_31_MAX {
            return Err(self.error("length exceeds PNG maximum"));
        }
        self.write_chunk_header(chunk_name, data.len() as u32)?;
        self.write_chunk_data(data)?;
        self.write_chunk_end()
    }

    /// Port of `png_write_chunk` (pngwutil.c#L209-L218), the API that takes a name as bytes.
    #[doc(alias = "png_write_chunk")]
    pub fn write_chunk(&mut self, chunk_string: &[u8], data: &[u8]) -> PngResult<()> {
        let name = u32::from_be_bytes([
            chunk_string[0],
            chunk_string[1],
            chunk_string[2],
            chunk_string[3],
        ]);
        self.write_complete_chunk(name, data)
    }

    /// Port of `png_image_size` (pngwutil.c#L221-L260), without the interlaced case.
    pub(crate) fn image_size(&self) -> usize {
        // Only return sizes up to the maximum of a png_uint_32; do this by limiting the width and
        // height used to 15 bits.
        let h = self.height;
        if self.rowbytes < 32768 && h < 32768 {
            (self.rowbytes + 1) * h as usize
        } else {
            0xffff_ffff
        }
    }

    /// Port of `png_deflate_claim` (pngwutil.c#L309-L452): takes the zlib stream for `owner`,
    /// creating or resetting it with the parameters for IDAT or for text, and shrinks the window
    /// for small data.
    #[doc(alias = "png_deflate_claim")]
    pub(crate) fn deflate_claim(&mut self, owner: u32, data_size: usize) -> ReturnCode {
        if self.zowner != 0 {
            // libpng 1.6.56 in a release build: a warning, then recovery.
            let msg = format!(
                "{}: {} using zstream",
                chunk_string(owner),
                chunk_string(self.zowner)
            );
            self.warning(&msg);
            // Attempt sane error recovery.
            if self.zowner == PNG_IDAT {
                // don't steal from IDAT
                self.zstream_msg_set("in use by IDAT");
                return ReturnCode::StreamError;
            }
            self.zowner = 0;
        }

        let level;
        let method;
        let mut window_bits;
        let mem_level;
        let strategy: i32;
        if owner == PNG_IDAT {
            level = self.w.zlib_level;
            method = self.w.zlib_method;
            window_bits = self.w.zlib_window_bits;
            mem_level = self.w.zlib_mem_level;
            strategy = if self.flags & flag::ZLIB_CUSTOM_STRATEGY != 0 {
                self.w.zlib_strategy
            } else if self.w.do_filter != PNG_FILTER_NONE {
                PNG_Z_DEFAULT_STRATEGY
            } else {
                // PNG_Z_DEFAULT_NOFILTER_STRATEGY
                0
            };
        } else {
            level = self.w.zlib_text_level;
            method = self.w.zlib_text_method;
            window_bits = self.w.zlib_text_window_bits;
            mem_level = self.w.zlib_text_mem_level;
            strategy = self.w.zlib_text_strategy;
        }

        // Adjust 'windowBits' down if larger than 'data_size'. Notice that zlib requires an extra
        // 262 bytes in the window in addition to the data to be able to see the whole of the
        // data, so if data_size+262 takes us to the next windowBits size we need to fix up the
        // value later. (Because even though deflate needs the extra window, inflate does not!)
        if data_size <= 16384 {
            let mut half_window_size: u32 = 1u32 << (window_bits - 1);
            while data_size + 262 <= half_window_size as usize {
                half_window_size >>= 1;
                window_bits -= 1;
            }
        }

        // Check against the previous initialized values, if any. libpng 1.6.56 never assigns
        // the zlib_set_* fields, so this always finds a change and ends the old stream.
        if self.w.zstream.is_some()
            && (self.w.zlib_set_level != level
                || self.w.zlib_set_method != method
                || self.w.zlib_set_window_bits != window_bits
                || self.w.zlib_set_mem_level != mem_level
                || self.w.zlib_set_strategy != strategy)
        {
            // deflateEnd: dropping the stream is the same as ending it.
            self.w.zstream = None;
        }

        // Now initialize if required, setting the new parameters, otherwise just do a simple
        // reset to the previous parameters.
        let ret = if let Some(zs) = self.w.zstream.as_mut() {
            zs.reset()
        } else if method != 8 {
            // deflateInit2 rejects a method other than Z_DEFLATED.
            ReturnCode::StreamError
        } else {
            match Deflate::new(level, window_bits, mem_level, strategy) {
                Ok(d) => {
                    self.w.zstream = Some(d);
                    ReturnCode::Ok
                }
                Err(e) => e,
            }
        };
        if ret == ReturnCode::Ok {
            self.zowner = owner;
        } else {
            self.zstream_error_set(ret);
        }
        ret
    }

    /// Port of `png_free_buffer_list` for the compression buffers: keeps only the first.
    fn free_buffer_list_tail(&mut self) {
        self.w.zbuffer_list.truncate(1);
    }

    /// Port of `png_text_compress` (pngwutil.c#L505-L651), with `png_text_compress_init`. The
    /// output is gathered into the blocks the C code uses (a 1024-byte first block, then blocks
    /// of `zbuffer_size`), and returned as one vector of `output_len` bytes.
    pub(crate) fn text_compress(
        &mut self,
        chunk_name: u32,
        input: &[u8],
        prefix_len: usize,
    ) -> PngResult<Vec<u8>> {
        let ret = self.deflate_claim(chunk_name, input.len());
        if ret != ReturnCode::Ok {
            return Err(self.zlib_error(ret));
        }

        let first_size: usize = 1024;
        let block_size = self.w.zbuffer_size;
        let mut blocks: Vec<Vec<u8>> = vec![vec![0u8; first_size]];
        let mut avail_out = first_size;
        let mut output_len = first_size;
        let mut input_pos = 0usize;
        let mut input_len = input.len();
        let mut ret;
        loop {
            let mut avail_in = ZLIB_IO_MAX;
            if avail_in > input_len {
                avail_in = input_len;
            }
            input_len -= avail_in;
            if avail_out == 0 {
                // Chunk data is limited to 2^31 bytes in length, so the prefix length must be
                // counted here.
                if output_len + prefix_len > PNG_UINT_31_MAX {
                    ret = ReturnCode::MemError;
                    break;
                }
                blocks.push(vec![0u8; block_size]);
                avail_out = block_size;
                output_len += avail_out;
            }
            let flush = if input_len > 0 {
                Flush::NoFlush
            } else {
                Flush::Finish
            };
            let block = blocks.last_mut().expect("at least one block");
            let start = block.len() - avail_out;
            let zs = self
                .w
                .zstream
                .as_mut()
                .expect("deflate_claim initialized the stream");
            let d = zs.deflate(
                &input[input_pos..input_pos + avail_in],
                &mut block[start..],
                flush,
            );
            input_pos += d.consumed;
            // Claw back input data that was not consumed.
            input_len += avail_in - d.consumed;
            avail_out -= d.produced;
            ret = d.ret;
            if ret != ReturnCode::Ok {
                break;
            }
        }

        // There may be some space left in the last output buffer. This needs to be subtracted
        // from output_len.
        output_len -= avail_out;
        // Now double check the output length, put in a custom message if it is too long.
        if output_len + prefix_len >= PNG_UINT_31_MAX {
            self.zstream_msg_set("compressed data too long");
            ret = ReturnCode::MemError;
        } else {
            self.zstream_error_set(ret);
        }

        // Reset zlib for another zTXt/iTXt or image data.
        self.zowner = 0;

        // The only success case is Z_STREAM_END, input_len must be 0; if not this is an internal
        // error.
        if ret == ReturnCode::StreamEnd && input_len == 0 {
            // Fix up the deflate header, if required.
            optimize_cmf(&mut blocks[0], input.len());
            let mut out = Vec::with_capacity(output_len);
            for b in &blocks {
                out.extend_from_slice(b);
            }
            out.truncate(output_len);
            // Z_OK is returned, not Z_STREAM_END.
            Ok(out)
        } else {
            Err(self.zlib_error(ret))
        }
    }

    /// Port of `png_compress_IDAT` (pngwutil.c#L932-L1070): compresses `input` into IDAT chunks
    /// of `zbuffer_size` bytes, flushing with `flush`. Each call keeps the stream state, so the
    /// same stream is fed row by row.
    #[doc(alias = "png_compress_IDAT")]
    pub(crate) fn compress_idat(&mut self, input: &[u8], flush: Flush) -> PngResult<()> {
        if self.zowner != PNG_IDAT {
            // First time. Ensure we have a temporary buffer for compression and trim the buffer
            // list if it has more than one entry to free memory.
            if self.w.zbuffer_list.is_empty() {
                let size = self.w.zbuffer_size;
                self.w.zbuffer_list.push(vec![0u8; size]);
            } else {
                self.free_buffer_list_tail();
            }
            let claim = self.deflate_claim(PNG_IDAT, self.image_size());
            if claim != ReturnCode::Ok {
                let msg = self.zlib_msg(claim);
                return Err(self.error(&msg));
            }
            // The output state is maintained in the stream, so it must be initialized here after
            // the claim.
            self.w.zout_pos = 0;
            self.w.zout_avail = self.w.zbuffer_size;
        }

        // Now loop reading and writing until all the input is consumed or an error terminates
        // the operation. The _out values are maintained across calls to this function, but the
        // input must be reset each time.
        let mut pos = 0usize;
        let mut input_len = input.len();
        loop {
            let mut avail = ZLIB_IO_MAX;
            if avail > input_len {
                avail = input_len; // safe because of the check
            }
            input_len -= avail;
            let flush_now = if input_len > 0 { Flush::NoFlush } else { flush };
            let out_pos = self.w.zout_pos;
            let out_avail = self.w.zout_avail;
            let out = &mut self.w.zbuffer_list[0][out_pos..out_pos + out_avail];
            let zs = self
                .w
                .zstream
                .as_mut()
                .expect("deflate_claim initialized the stream");
            let d = zs.deflate(&input[pos..pos + avail], out, flush_now);
            pos += d.consumed;
            // Claw back the input that was not consumed.
            input_len += avail - d.consumed;
            self.w.zout_pos += d.produced;
            self.w.zout_avail -= d.produced;
            let ret = d.ret;

            // OUTPUT: write complete IDAT chunks when avail_out drops to zero.
            if self.w.zout_avail == 0 {
                let mut data = std::mem::take(&mut self.w.zbuffer_list[0]);
                let size = self.w.zbuffer_size;
                // Write an IDAT containing the data then reset the buffer. The first IDAT may
                // need deflate header optimization.
                if self.mode & mode::HAVE_IDAT == 0
                    && self.compression_type == PNG_COMPRESSION_TYPE_BASE
                {
                    let image = self.image_size();
                    optimize_cmf(&mut data, image);
                }
                let r = if size > 0 {
                    self.write_complete_chunk(PNG_IDAT, &data[..size])
                } else {
                    Ok(())
                };
                self.w.zbuffer_list[0] = data;
                r?;
                self.mode |= mode::HAVE_IDAT;
                self.w.zout_pos = 0;
                self.w.zout_avail = size;
                // For SYNC_FLUSH or FINISH it is essential to keep calling zlib with the same
                // flush parameter until it has finished output, for NO_FLUSH it doesn't matter.
                if ret == ReturnCode::Ok && flush != Flush::NoFlush {
                    continue;
                }
            }

            // The order of these checks doesn't matter much; it just affects which possible
            // error might be detected if multiple things go wrong at once.
            if ret == ReturnCode::Ok {
                // most likely return code! If all the input has been consumed then just return.
                // If Z_FINISH was used as the flush parameter something has gone wrong if we get
                // here.
                if input_len == 0 {
                    if flush == Flush::Finish {
                        return Err(self.error("Z_OK on Z_FINISH with output space"));
                    }
                    return Ok(());
                }
            } else if ret == ReturnCode::StreamEnd && flush == Flush::Finish {
                // This is the end of the IDAT data; any pending output must be flushed. For small
                // PNG files we may still be at the beginning.
                let mut data = std::mem::take(&mut self.w.zbuffer_list[0]);
                let size = self.w.zbuffer_size - self.w.zout_avail;
                if self.mode & mode::HAVE_IDAT == 0
                    && self.compression_type == PNG_COMPRESSION_TYPE_BASE
                {
                    let image = self.image_size();
                    optimize_cmf(&mut data, image);
                }
                let r = if size > 0 {
                    self.write_complete_chunk(PNG_IDAT, &data[..size])
                } else {
                    Ok(())
                };
                self.w.zbuffer_list[0] = data;
                r?;
                self.w.zout_avail = 0;
                self.mode |= mode::HAVE_IDAT | mode::AFTER_IDAT;
                self.zowner = 0; // Release the stream
                return Ok(());
            } else {
                self.zstream_error_set(ret);
                let msg = self.zlib_msg(ret);
                return Err(self.error(&msg));
            }
        }
    }

    /// Port of `png_write_IHDR` (pngwutil.c#L689-L857). Interlaced images are rejected: the port
    /// does not carry Adam7 writing.
    #[doc(alias = "png_write_IHDR")]
    pub(crate) fn write_ihdr(
        &mut self,
        width: u32,
        height: u32,
        bit_depth: u8,
        color_type: u8,
        compression_type: u8,
        filter_type: u8,
        interlace_type: u8,
    ) -> PngResult<()> {
        let channels: u8 = match color_type {
            0 => match bit_depth {
                1 | 2 | 4 | 8 | 16 => 1,
                _ => return Err(self.error("Invalid bit depth for grayscale image")),
            },
            2 => {
                if bit_depth != 8 && bit_depth != 16 {
                    return Err(self.error("Invalid bit depth for RGB image"));
                }
                3
            }
            3 => match bit_depth {
                1 | 2 | 4 | 8 => 1,
                _ => return Err(self.error("Invalid bit depth for paletted image")),
            },
            4 => {
                if bit_depth != 8 && bit_depth != 16 {
                    return Err(self.error("Invalid bit depth for grayscale+alpha image"));
                }
                2
            }
            6 => {
                if bit_depth != 8 && bit_depth != 16 {
                    return Err(self.error("Invalid bit depth for RGBA image"));
                }
                4
            }
            _ => return Err(self.error("Invalid image color type specified")),
        };

        let mut compression_type = compression_type;
        if compression_type != PNG_COMPRESSION_TYPE_BASE {
            self.warning("Invalid compression type specified");
            compression_type = PNG_COMPRESSION_TYPE_BASE;
        }

        // The MNG filter method 64 is never permitted (no png_permit_mng_features here).
        let mut filter_type = filter_type;
        if filter_type != 0 {
            self.warning("Invalid filter type specified");
            filter_type = 0;
        }

        if interlace_type != 0 {
            return Err(self.error("interlaced PNG writing is not ported"));
        }

        self.bit_depth = bit_depth;
        self.color_type = color_type;
        self.interlaced = 0;
        self.filter_type = filter_type;
        self.compression_type = compression_type;
        self.width = width;
        self.height = height;
        self.channels = channels;
        self.pixel_depth = bit_depth * channels;
        self.rowbytes = crate::structs::rowbytes(self.pixel_depth, width);
        self.w.usr_width = self.width;
        self.w.usr_bit_depth = self.bit_depth;
        self.w.usr_channels = self.channels;

        let mut buf = [0u8; 13];
        buf[..4].copy_from_slice(&save_uint_32(width));
        buf[4..8].copy_from_slice(&save_uint_32(height));
        buf[8] = bit_depth;
        buf[9] = color_type;
        buf[10] = compression_type;
        buf[11] = filter_type;
        buf[12] = 0;
        self.write_complete_chunk(PNG_IHDR, &buf)?;

        if self.w.do_filter == PNG_NO_FILTERS {
            self.w.do_filter = if self.color_type == PNG_COLOR_TYPE_PALETTE || self.bit_depth < 8 {
                PNG_FILTER_NONE
            } else {
                PNG_ALL_FILTERS
            };
        }
        self.mode = mode::HAVE_IHDR; // not READY_FOR_ZTXT
        Ok(())
    }

    /// Port of `png_write_PLTE` (pngwutil.c#L859-L929).
    pub(crate) fn write_plte(&mut self, palette: &[PngColor]) -> PngResult<()> {
        let num_pal = palette.len();
        let max_palette_length = if self.color_type == PNG_COLOR_TYPE_PALETTE {
            1usize << self.bit_depth
        } else {
            PNG_MAX_PALETTE_LENGTH
        };
        if num_pal == 0 || num_pal > max_palette_length {
            if self.color_type == PNG_COLOR_TYPE_PALETTE {
                return Err(self.error("Invalid number of colors in palette"));
            }
            self.warning("Invalid number of colors in palette");
            return Ok(());
        }
        if self.color_type & PNG_COLOR_MASK_COLOR == 0 {
            self.warning("Ignoring request to write a PLTE chunk in grayscale PNG");
            return Ok(());
        }
        self.num_palette = num_pal as u32;
        self.write_chunk_header(PNG_PLTE, (num_pal * 3) as u32)?;
        for c in palette {
            self.write_chunk_data(&[c.red, c.green, c.blue])?;
        }
        self.write_chunk_end()?;
        self.mode |= mode::HAVE_PLTE;
        Ok(())
    }

    /// Port of `png_write_sRGB` (pngwutil.c#L1100-L1113).
    pub(crate) fn write_srgb(&mut self, srgb_intent: i32) -> PngResult<()> {
        if srgb_intent >= PNG_SRGB_INTENT_LAST {
            self.warning("Invalid sRGB rendering intent specified");
        }
        self.write_complete_chunk(PNG_SRGB, &[srgb_intent as u8])
    }

    /// Port of `png_write_iCCP` (pngwutil.c#L1118-L1177). The profile is checked as libpng does
    /// when it is written, then deflated with the text settings.
    pub(crate) fn write_iccp(&mut self, name: &str, profile: &[u8]) -> PngResult<()> {
        let profile_len = profile.len();
        // These are all internal problems: the profile should have been checked before when it
        // was stored.
        if profile_len < 132 {
            return Err(self.error("ICC profile too short"));
        }
        if get_uint_32(profile) as usize != profile_len {
            return Err(self.error("Incorrect data in iCCP"));
        }
        let temp = u32::from(profile[8]);
        if temp > 3 && (profile_len & 0x03) != 0 {
            return Err(self.error("ICC profile length invalid (not a multiple of 4)"));
        }

        let (key_len, mut new_name) = check_keyword(name.as_bytes());
        if key_len == 0 {
            return Err(self.error("iCCP: invalid keyword"));
        }
        // The keyword terminator and the compression method byte.
        new_name.truncate(key_len + 1);
        new_name.push(PNG_COMPRESSION_TYPE_BASE);
        let name_len = new_name.len();

        let output = self.text_compress(PNG_ICCP, profile, name_len)?;
        self.write_chunk_header(PNG_ICCP, (name_len + output.len()) as u32)?;
        self.write_chunk_data(&new_name)?;
        self.write_chunk_data(&output)?;
        self.write_chunk_end()
    }

    /// Port of `png_write_sBIT` (pngwutil.c#L1235-L1290).
    pub(crate) fn write_sbit(&mut self, sbit: PngColor8, color_type: u8) -> PngResult<()> {
        let mut buf = [0u8; 4];
        let mut size;
        // Make sure we don't depend upon the order of PNG_COLOR_8.
        if color_type & PNG_COLOR_MASK_COLOR != 0 {
            let maxbits = if color_type == PNG_COLOR_TYPE_PALETTE {
                8
            } else {
                self.w.usr_bit_depth
            };
            if sbit.red == 0
                || sbit.red > maxbits
                || sbit.green == 0
                || sbit.green > maxbits
                || sbit.blue == 0
                || sbit.blue > maxbits
            {
                self.warning("Invalid sBIT depth specified");
                return Ok(());
            }
            buf[0] = sbit.red;
            buf[1] = sbit.green;
            buf[2] = sbit.blue;
            size = 3;
        } else {
            if sbit.gray == 0 || sbit.gray > self.w.usr_bit_depth {
                self.warning("Invalid sBIT depth specified");
                return Ok(());
            }
            buf[0] = sbit.gray;
            size = 1;
        }
        if color_type & PNG_COLOR_MASK_ALPHA != 0 {
            if sbit.alpha == 0 || sbit.alpha > self.w.usr_bit_depth {
                self.warning("Invalid sBIT depth specified");
                return Ok(());
            }
            buf[size] = sbit.alpha;
            size += 1;
        }
        self.write_complete_chunk(PNG_SBIT, &buf[..size])
    }

    /// Port of `png_write_tEXt` (pngwutil.c#L1569-L1608).
    pub(crate) fn write_text_chunk(&mut self, key: &[u8], text: &[u8]) -> PngResult<()> {
        let (key_len, new_key) = check_keyword(key);
        if key_len == 0 {
            return Err(self.error("tEXt: invalid keyword"));
        }
        // The text is a C string: its length is strlen(text).
        let text = match text.iter().position(|&b| b == 0) {
            Some(end) => &text[..end],
            None => text,
        };
        let text_len = text.len();
        if text_len > PNG_UINT_31_MAX - (key_len + 1) {
            return Err(self.error("tEXt: text too long"));
        }
        // Make sure we include the 0 after the key.
        self.write_chunk_header(PNG_TEXT, (key_len + text_len + 1) as u32)?;
        // We leave it to the application to meet the PNG requirements on the contents of the text.
        self.write_chunk_data(&new_key[..=key_len])?;
        if text_len != 0 {
            self.write_chunk_data(text)?;
        }
        self.write_chunk_end()
    }

    /// Port of `png_write_zTXt` (pngwutil.c#L1612-L1660) for the deflate compression type.
    pub(crate) fn write_ztxt(&mut self, key: &[u8], text: &[u8]) -> PngResult<()> {
        let (key_len, mut new_key) = check_keyword(key);
        if key_len == 0 {
            return Err(self.error("zTXt: invalid keyword"));
        }
        new_key.truncate(key_len + 1);
        new_key.push(PNG_COMPRESSION_TYPE_BASE);
        let prefix = new_key.len();
        let text = match text.iter().position(|&b| b == 0) {
            Some(end) => &text[..end],
            None => text,
        };
        let output = self.text_compress(PNG_ZTXT, text, prefix)?;
        self.write_chunk_header(PNG_ZTXT, (prefix + output.len()) as u32)?;
        self.write_chunk_data(&new_key)?;
        self.write_chunk_data(&output)?;
        self.write_chunk_end()
    }

    /// Port of `png_write_find_filter` (pngwutil.c#L2549-L2744): picks the filter for the row in
    /// `row_buf` with the smallest sum of absolute values, then writes the filtered row. The
    /// winner is tracked as a buffer, since a winning try row is swapped with `tst_row` (the C
    /// code keeps the pointer `best_row`, which follows the swap).
    pub(crate) fn write_find_filter(&mut self, rowbytes: usize, pixel_depth: u8) -> PngResult<()> {
        let mut filter_to_do = u32::from(self.w.do_filter);
        let bpp = (usize::from(pixel_depth) + 7) >> 3;
        let row_bytes = rowbytes;
        let mut mins: usize = usize::MAX - 256;
        let mut best = Best::Row;

        // We don't need to test the 'no filter' case if this is the only filter that has been
        // chosen, as it doesn't actually do anything to the data.
        if usize::MAX / 128 <= row_bytes {
            // Overflow can occur in the calculation, just select the lowest set filter.
            filter_to_do &= 0u32.wrapping_sub(filter_to_do);
        } else if filter_to_do & u32::from(PNG_FILTER_NONE) != 0
            && filter_to_do != u32::from(PNG_FILTER_NONE)
        {
            // Overflow not possible and multiple filters in the list, including the 'none' filter.
            let mut sum = 0usize;
            for i in 0..row_bytes {
                sum += abs_sum_term(self.w.row_buf[1 + i]);
            }
            mins = sum;
        }

        if filter_to_do == u32::from(PNG_FILTER_SUB) {
            setup_sub_row_only(&mut self.w.try_row, &self.w.row_buf, bpp, row_bytes);
            best = Best::Try;
        } else if filter_to_do & u32::from(PNG_FILTER_SUB) != 0 {
            let sum = setup_sub_row(&mut self.w.try_row, &self.w.row_buf, bpp, row_bytes, mins);
            if sum < mins {
                mins = sum;
                best = take_try(&mut self.w.try_row, &mut self.w.tst_row);
            }
        }

        if filter_to_do == u32::from(PNG_FILTER_UP) {
            setup_up_row_only(
                &mut self.w.try_row,
                &self.w.row_buf,
                &self.w.prev_row,
                row_bytes,
            );
            best = Best::Try;
        } else if filter_to_do & u32::from(PNG_FILTER_UP) != 0 {
            let sum = setup_up_row(
                &mut self.w.try_row,
                &self.w.row_buf,
                &self.w.prev_row,
                row_bytes,
                mins,
            );
            if sum < mins {
                mins = sum;
                best = take_try(&mut self.w.try_row, &mut self.w.tst_row);
            }
        }

        if filter_to_do == u32::from(PNG_FILTER_AVG) {
            setup_avg_row_only(
                &mut self.w.try_row,
                &self.w.row_buf,
                &self.w.prev_row,
                bpp,
                row_bytes,
            );
            best = Best::Try;
        } else if filter_to_do & u32::from(PNG_FILTER_AVG) != 0 {
            let sum = setup_avg_row(
                &mut self.w.try_row,
                &self.w.row_buf,
                &self.w.prev_row,
                bpp,
                row_bytes,
                mins,
            );
            if sum < mins {
                mins = sum;
                best = take_try(&mut self.w.try_row, &mut self.w.tst_row);
            }
        }

        if filter_to_do == u32::from(PNG_FILTER_PAETH) {
            setup_paeth_row_only(
                &mut self.w.try_row,
                &self.w.row_buf,
                &self.w.prev_row,
                bpp,
                row_bytes,
            );
            best = Best::Try;
        } else if filter_to_do & u32::from(PNG_FILTER_PAETH) != 0 {
            let sum = setup_paeth_row(
                &mut self.w.try_row,
                &self.w.row_buf,
                &self.w.prev_row,
                bpp,
                row_bytes,
                mins,
            );
            if sum < mins {
                best = take_try(&mut self.w.try_row, &mut self.w.tst_row);
            }
        }

        self.write_filtered_row(best, rowbytes + 1)
    }

    /// Port of `png_write_filtered_row` (pngwutil.c#L2746-L2778): compresses the chosen row,
    /// rotates the previous-row buffer, and finishes the row.
    fn write_filtered_row(&mut self, best: Best, full_row_length: usize) -> PngResult<()> {
        let src = match best {
            Best::Row => std::mem::take(&mut self.w.row_buf),
            Best::Try => std::mem::take(&mut self.w.try_row),
            Best::Tst => std::mem::take(&mut self.w.tst_row),
        };
        let r = self.compress_idat(&src[..full_row_length], Flush::NoFlush);
        match best {
            Best::Row => self.w.row_buf = src,
            Best::Try => self.w.try_row = src,
            Best::Tst => self.w.tst_row = src,
        }
        r?;
        if !self.w.prev_row.is_empty() {
            std::mem::swap(&mut self.w.prev_row, &mut self.w.row_buf);
        }
        self.write_finish_row()?;
        self.w.flush_rows += 1;
        if self.w.flush_dist > 0 && self.w.flush_rows >= self.w.flush_dist {
            self.write_flush()?;
        }
        Ok(())
    }

    /// Port of `png_zstream_error` and the message part of `png_error(zstream.msg)`.
    fn zlib_msg(&self, ret: ReturnCode) -> String {
        self.zstream_message_for(ret).to_owned()
    }

    /// The `png_error` for a failed zlib call, with the message zlib (or libpng) gives.
    fn zlib_error(&mut self, ret: ReturnCode) -> crate::error::PngError {
        let msg = self.zlib_msg(ret);
        self.error(&msg)
    }

    /// Records the zlib message for `ret` the way `png_zstream_error` does.
    fn zstream_error_set(&mut self, ret: ReturnCode) {
        let msg = self.zstream_message_for(ret);
        self.zstream_msg_set(msg);
    }

    fn zstream_msg_set(&mut self, msg: &'static str) {
        self.w.zstream_msg = Some(msg);
    }

    fn zstream_message_for(&self, ret: ReturnCode) -> &'static str {
        if let Some(msg) = self.w.zstream_msg {
            return msg;
        }
        match ret {
            ReturnCode::Ok => "unexpected zlib return code",
            ReturnCode::StreamEnd => "unexpected end of LZ stream",
            ReturnCode::NeedDict => "missing LZ dictionary",
            ReturnCode::DataError => "LZ data error",
            ReturnCode::StreamError => "LZ stream error",
            ReturnCode::MemError => "zlib memory error",
            ReturnCode::BufError => "zlib buffer error",
        }
    }
}

/// Which buffer holds the chosen row: `row_buf` (unfiltered, filter type none), or one of the two
/// try rows. This is the `best_row` pointer of `png_write_find_filter`.
#[derive(Clone, Copy)]
enum Best {
    Row,
    Try,
    Tst,
}

/// The winning-candidate step of `png_write_find_filter`: when a try row wins and there is a
/// second try row, the two are exchanged, so the winner moves to `tst_row`.
fn take_try(try_row: &mut Vec<u8>, tst_row: &mut Vec<u8>) -> Best {
    if tst_row.is_empty() {
        Best::Try
    } else {
        std::mem::swap(try_row, tst_row);
        Best::Tst
    }
}

/// Port of `png_setup_sub_row` (pngwutil.c#L2278-L2315). `row` is `row_buf`, so pixel `i` is at
/// `row[1 + i]`; the try row is written from index 1 with its filter type at index 0.
fn setup_sub_row(
    try_row: &mut [u8],
    row: &[u8],
    bpp: usize,
    row_bytes: usize,
    lmins: usize,
) -> usize {
    try_row[0] = PNG_FILTER_VALUE_SUB;
    let mut sum = 0usize;
    let mut i = 0usize;
    while i < bpp {
        let v = row[1 + i];
        try_row[1 + i] = v;
        sum += abs_sum_term(v);
        i += 1;
    }
    while i < row_bytes {
        let v = row[1 + i].wrapping_sub(row[1 + i - bpp]);
        try_row[1 + i] = v;
        sum += abs_sum_term(v);
        if sum > lmins {
            // We are already worse, don't continue.
            break;
        }
        i += 1;
    }
    sum
}

/// Port of `png_setup_sub_row_only` (pngwutil.c#L2317-L2337).
fn setup_sub_row_only(try_row: &mut [u8], row: &[u8], bpp: usize, row_bytes: usize) {
    try_row[0] = PNG_FILTER_VALUE_SUB;
    let mut i = 0usize;
    while i < bpp {
        try_row[1 + i] = row[1 + i];
        i += 1;
    }
    while i < row_bytes {
        try_row[1 + i] = row[1 + i].wrapping_sub(row[1 + i - bpp]);
        i += 1;
    }
}

/// Port of `png_setup_up_row` (pngwutil.c#L2339-L2364).
fn setup_up_row(
    try_row: &mut [u8],
    row: &[u8],
    prev: &[u8],
    row_bytes: usize,
    lmins: usize,
) -> usize {
    try_row[0] = PNG_FILTER_VALUE_UP;
    let mut sum = 0usize;
    for i in 0..row_bytes {
        let v = row[1 + i].wrapping_sub(prev[1 + i]);
        try_row[1 + i] = v;
        sum += abs_sum_term(v);
        if sum > lmins {
            // We are already worse, don't continue.
            break;
        }
    }
    sum
}

/// Port of `png_setup_up_row_only` (pngwutil.c#L2366-L2380).
fn setup_up_row_only(try_row: &mut [u8], row: &[u8], prev: &[u8], row_bytes: usize) {
    try_row[0] = PNG_FILTER_VALUE_UP;
    for i in 0..row_bytes {
        try_row[1 + i] = row[1 + i].wrapping_sub(prev[1 + i]);
    }
}

/// Port of `png_setup_avg_row` (pngwutil.c#L2382-L2420). The average is `(a + b) / 2` with C's
/// truncating integer division on non-negative values.
fn setup_avg_row(
    try_row: &mut [u8],
    row: &[u8],
    prev: &[u8],
    bpp: usize,
    row_bytes: usize,
    lmins: usize,
) -> usize {
    try_row[0] = PNG_FILTER_VALUE_AVG;
    let mut sum = 0usize;
    let mut i = 0usize;
    while i < bpp {
        let v = (i32::from(row[1 + i]) - i32::from(prev[1 + i]) / 2) as u8;
        try_row[1 + i] = v;
        sum += abs_sum_term(v);
        i += 1;
    }
    while i < row_bytes {
        let left = i32::from(row[1 + i - bpp]);
        let v = (i32::from(row[1 + i]) - i32::midpoint(i32::from(prev[1 + i]), left)) as u8;
        try_row[1 + i] = v;
        sum += abs_sum_term(v);
        if sum > lmins {
            // We are already worse, don't continue.
            break;
        }
        i += 1;
    }
    sum
}

/// Port of `png_setup_avg_row_only` (pngwutil.c#L2422-L2442).
fn setup_avg_row_only(try_row: &mut [u8], row: &[u8], prev: &[u8], bpp: usize, row_bytes: usize) {
    try_row[0] = PNG_FILTER_VALUE_AVG;
    let mut i = 0usize;
    while i < bpp {
        try_row[1 + i] = (i32::from(row[1 + i]) - i32::from(prev[1 + i]) / 2) as u8;
        i += 1;
    }
    while i < row_bytes {
        let left = i32::from(row[1 + i - bpp]);
        try_row[1 + i] =
            (i32::from(row[1 + i]) - i32::midpoint(i32::from(prev[1 + i]), left)) as u8;
        i += 1;
    }
}

/// Port of the Paeth predictor of `png_setup_paeth_row` (pngwutil.c#L2444-L2503): `a` is the
/// left pixel, `b` the one above, `c` the one above and to the left.
#[inline]
fn paeth_predictor(a: i32, b: i32, c: i32) -> i32 {
    let p = b - c;
    let pc0 = a - c;
    let pa = p.abs();
    let pb = pc0.abs();
    let pc = (p + pc0).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// Port of `png_setup_paeth_row` (pngwutil.c#L2444-L2503).
fn setup_paeth_row(
    try_row: &mut [u8],
    row: &[u8],
    prev: &[u8],
    bpp: usize,
    row_bytes: usize,
    lmins: usize,
) -> usize {
    try_row[0] = PNG_FILTER_VALUE_PAETH;
    let mut sum = 0usize;
    let mut i = 0usize;
    while i < bpp {
        let v = row[1 + i].wrapping_sub(prev[1 + i]);
        try_row[1 + i] = v;
        sum += abs_sum_term(v);
        i += 1;
    }
    while i < row_bytes {
        let a = i32::from(row[1 + i - bpp]);
        let b = i32::from(prev[1 + i]);
        let c = i32::from(prev[1 + i - bpp]);
        let p = paeth_predictor(a, b, c);
        let v = row[1 + i].wrapping_sub(p as u8);
        try_row[1 + i] = v;
        sum += abs_sum_term(v);
        if sum > lmins {
            // We are already worse, don't continue.
            break;
        }
        i += 1;
    }
    sum
}

/// Port of `png_setup_paeth_row_only` (pngwutil.c#L2505-L2547).
fn setup_paeth_row_only(try_row: &mut [u8], row: &[u8], prev: &[u8], bpp: usize, row_bytes: usize) {
    try_row[0] = PNG_FILTER_VALUE_PAETH;
    let mut i = 0usize;
    while i < bpp {
        try_row[1 + i] = row[1 + i].wrapping_sub(prev[1 + i]);
        i += 1;
    }
    while i < row_bytes {
        let a = i32::from(row[1 + i - bpp]);
        let b = i32::from(prev[1 + i]);
        let c = i32::from(prev[1 + i - bpp]);
        let p = paeth_predictor(a, b, c);
        try_row[1 + i] = row[1 + i].wrapping_sub(p as u8);
        i += 1;
    }
}

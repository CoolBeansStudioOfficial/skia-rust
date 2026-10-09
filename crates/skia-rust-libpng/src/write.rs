// Copyright (C) 1998-2026 Glenn Randers-Pehrson and the libpng contributors.
// Copyright (C) 2026 The skia-rust Authors.
// Use of this source code is governed by the libpng licence (libpng-2.0) in the LICENSE file.
// Port of: pngwrite.c, and the write-side `png_set_*` functions of pngwrite.c, pngset.c and
// pngtrans.c that Skia's PNG encoder calls (libpng 1.6.56, skia.googlesource.com/third_party/
// libpng@d5515b5b). Written with the configuration of scripts/pnglibconf.h.prebuilt: all write
// features on except interlacing, which Skia never uses (see the crate documentation).

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
    clippy::unused_self
)]

use skia_rust_zlib::{Deflate, Flush};

use crate::error::PngResult;
use crate::png::{
    PNG_USER_CHUNK_CACHE_MAX, PNG_USER_CHUNK_MALLOC_MAX, PNG_USER_HEIGHT_MAX, PNG_USER_WIDTH_MAX,
};
use crate::rtran::RowInfo;
use crate::structs::{PNG_IEND, PngInfo, PngStruct, flag, info, mode};
use crate::wutil::{
    PNG_FILTER_AVG, PNG_FILTER_NONE, PNG_FILTER_PAETH, PNG_FILTER_SUB, PNG_FILTER_UP,
};

/// Port of `PNG_ZBUF_SIZE` (scripts/pnglibconf.h.prebuilt#L225): the size of one IDAT buffer.
pub(crate) const PNG_ZBUF_SIZE: usize = 8192;
/// Port of `PNG_Z_DEFAULT_COMPRESSION` and `PNG_Z_DEFAULT_STRATEGY` (pnglibconf.h.prebuilt).
pub(crate) const PNG_Z_DEFAULT_COMPRESSION: i32 = -1;
pub(crate) const PNG_Z_DEFAULT_STRATEGY: i32 = 1;
/// Port of `PNG_TEXT_Z_DEFAULT_STRATEGY` (pnglibconf.h.prebuilt#L220).
pub(crate) const PNG_TEXT_Z_DEFAULT_STRATEGY: i32 = 0;
/// Port of `PNG_NO_FILTERS` (png.h).
pub(crate) const PNG_NO_FILTERS: u8 = 0x00;
/// Port of `PNG_ALL_FILTERS` (png.h).
pub(crate) const PNG_ALL_FILTERS: u8 = 0xf8;

/// The output callback of a write struct: receives each piece of the PNG stream and returns
/// `false` to fail the write (libpng's `png_rw_ptr` with the error turned into a result).
pub type PngWriteFn = Box<dyn FnMut(&[u8]) -> bool>;

/// The write-side state of `png_struct` (pngstruct.h, the fields the write path reads and
/// writes). The shared fields (`width`, `height`, `bit_depth`, `color_type`, `channels`,
/// `pixel_depth`, `rowbytes`, `filter_type`, `compression_type`, `transformations`, `mode`,
/// `flags`, `zowner`, `crc`) live on [`PngStruct`] as they do in libpng.
pub(crate) struct WriteState {
    /// Port of `write_data_fn`: receives each piece of output. Returns `false` on failure, which
    /// libpng turns into a `png_error`.
    pub(crate) write_fn: Option<PngWriteFn>,
    /// Port of `zstream` for the write side. `None` until `png_deflate_claim` initializes it.
    pub(crate) zstream: Option<Deflate>,
    /// Port of `zlib_level`, `zlib_method`, `zlib_window_bits`, `zlib_mem_level`,
    /// `zlib_strategy` (IDAT) and their `zlib_text_*` counterparts.
    pub(crate) zlib_level: i32,
    pub(crate) zlib_method: i32,
    pub(crate) zlib_window_bits: i32,
    pub(crate) zlib_mem_level: i32,
    pub(crate) zlib_strategy: i32,
    pub(crate) zlib_text_level: i32,
    pub(crate) zlib_text_method: i32,
    pub(crate) zlib_text_window_bits: i32,
    pub(crate) zlib_text_mem_level: i32,
    pub(crate) zlib_text_strategy: i32,
    /// Port of `zlib_set_level` and friends. libpng 1.6.56 compares them in `png_deflate_claim`
    /// but never assigns them, so they stay zero (the calloc'd value) and the comparison always
    /// finds a change. The port keeps that behaviour.
    pub(crate) zlib_set_level: i32,
    pub(crate) zlib_set_method: i32,
    pub(crate) zlib_set_window_bits: i32,
    pub(crate) zlib_set_mem_level: i32,
    pub(crate) zlib_set_strategy: i32,
    /// Port of `zbuffer_size` and `zbuffer_list`. `zbuffer_list[0]` is the IDAT output buffer.
    pub(crate) zbuffer_size: usize,
    pub(crate) zbuffer_list: Vec<Vec<u8>>,
    /// Port of `zstream.next_out` (as an offset into `zbuffer_list[0]`) and `zstream.avail_out`
    /// for the IDAT stream, which persist across `png_compress_IDAT` calls.
    pub(crate) zout_pos: usize,
    pub(crate) zout_avail: usize,
    /// Port of `do_filter`: the filters to try (`PNG_FILTER_*` bits, or one of them).
    pub(crate) do_filter: u8,
    /// Port of `row_buf`, `prev_row`, `try_row` and `tst_row`. An empty vector is a NULL pointer.
    pub(crate) row_buf: Vec<u8>,
    pub(crate) prev_row: Vec<u8>,
    pub(crate) try_row: Vec<u8>,
    pub(crate) tst_row: Vec<u8>,
    /// Port of `usr_width`, `usr_channels` and `usr_bit_depth`: the application's row layout.
    pub(crate) usr_width: u32,
    pub(crate) usr_channels: u8,
    pub(crate) usr_bit_depth: u8,
    /// Port of `row_number`, `num_rows` and `pass`.
    pub(crate) row_number: u32,
    pub(crate) num_rows: u32,
    pub(crate) pass: u32,
    /// Port of `transformed_pixel_depth` and `maximum_pixel_depth`.
    pub(crate) transformed_pixel_depth: u8,
    pub(crate) maximum_pixel_depth: u8,
    /// Port of `flush_dist` and `flush_rows`.
    pub(crate) flush_dist: u32,
    pub(crate) flush_rows: u32,
    /// Port of `zstream.msg` for the write side: the message of the last zlib failure.
    pub(crate) zstream_msg: Option<&'static str>,
}

impl Default for WriteState {
    /// Port of the write defaults of `png_create_write_struct` (pngwrite.c#L558-L620).
    fn default() -> Self {
        WriteState {
            write_fn: None,
            zstream: None,
            zlib_level: PNG_Z_DEFAULT_COMPRESSION,
            zlib_method: 8,
            zlib_window_bits: 15,
            zlib_mem_level: 8,
            zlib_strategy: PNG_Z_DEFAULT_STRATEGY,
            zlib_text_level: PNG_Z_DEFAULT_COMPRESSION,
            zlib_text_method: 8,
            zlib_text_window_bits: 15,
            zlib_text_mem_level: 8,
            zlib_text_strategy: PNG_TEXT_Z_DEFAULT_STRATEGY,
            zlib_set_level: 0,
            zlib_set_method: 0,
            zlib_set_window_bits: 0,
            zlib_set_mem_level: 0,
            zlib_set_strategy: 0,
            zbuffer_size: PNG_ZBUF_SIZE,
            zbuffer_list: Vec::new(),
            zout_pos: 0,
            zout_avail: 0,
            do_filter: PNG_NO_FILTERS,
            row_buf: Vec::new(),
            prev_row: Vec::new(),
            try_row: Vec::new(),
            tst_row: Vec::new(),
            usr_width: 0,
            usr_channels: 0,
            usr_bit_depth: 0,
            row_number: 0,
            num_rows: 0,
            pass: 0,
            transformed_pixel_depth: 0,
            maximum_pixel_depth: 0,
            flush_dist: 0,
            flush_rows: 0,
            zstream_msg: None,
        }
    }
}

impl PngStruct {
    /// Port of `png_create_write_struct` followed by `png_set_write_fn` (pngwrite.c#L544-L620,
    /// pngwio.c). `write_fn` receives each piece of output and returns `false` to fail the write.
    #[doc(alias = "png_create_write_struct")]
    #[must_use]
    pub fn new_write(write_fn: PngWriteFn) -> Self {
        let mut p = PngStruct {
            // Port of PNG_FLAG_APP_WARNINGS_WARN in a release build (pngwrite.c#L609-L611), and
            // the user limits that png_create_png_struct sets for every struct.
            flags: flag::APP_WARNINGS_WARN,
            user_width_max: PNG_USER_WIDTH_MAX,
            user_height_max: PNG_USER_HEIGHT_MAX,
            user_chunk_cache_max: PNG_USER_CHUNK_CACHE_MAX,
            user_chunk_malloc_max: PNG_USER_CHUNK_MALLOC_MAX,
            ..PngStruct::default()
        };
        p.w.write_fn = Some(write_fn);
        p
    }

    /// Port of `png_set_flush` (pngwrite.c#L956-L960).
    #[doc(alias = "png_set_flush")]
    pub fn set_flush(&mut self, nrows: i32) {
        self.w.flush_dist = if nrows < 0 { 0 } else { nrows as u32 };
    }

    /// Port of `png_write_flush` (pngwrite.c#L968-L983).
    #[doc(alias = "png_write_flush")]
    pub fn write_flush(&mut self) -> PngResult<()> {
        if self.w.row_number >= self.w.num_rows {
            return Ok(());
        }
        self.compress_idat(&[], Flush::SyncFlush)?;
        self.w.flush_rows = 0;
        Ok(())
    }

    /// Port of `png_set_filter` (pngwrite.c#L1058-L1181). Only the base filter method exists.
    /// As in libpng 1.6.56, the switch on the filter value only warns: the stored value is
    /// `filters` itself, after the row-buffer checks.
    #[doc(alias = "png_set_filter")]
    pub fn set_filter(&mut self, method: i32, filters: i32) -> PngResult<()> {
        if method != 0 {
            return Err(self.error("Unknown custom filter method"));
        }
        let mut filters = filters as u32;
        if matches!(filters & (u32::from(PNG_ALL_FILTERS) | 0x07), 5..=7) {
            // png_app_error: a warning in a release build.
            self.warning("Unknown row filter for method 0");
        }
        if !self.w.row_buf.is_empty() {
            // The row buffers are allocated after the start of the image, so the filters that
            // need the previous row cannot be added now.
            if self.height == 1 {
                filters &= !(u32::from(PNG_FILTER_UP)
                    | u32::from(PNG_FILTER_AVG)
                    | u32::from(PNG_FILTER_PAETH));
            }
            if self.width == 1 {
                filters &= !(u32::from(PNG_FILTER_SUB)
                    | u32::from(PNG_FILTER_AVG)
                    | u32::from(PNG_FILTER_PAETH));
            }
            let late =
                u32::from(PNG_FILTER_UP) | u32::from(PNG_FILTER_AVG) | u32::from(PNG_FILTER_PAETH);
            if filters & late != 0 && self.w.prev_row.is_empty() {
                self.warning("png_set_filter: UP/AVG/PAETH cannot be added after start");
                filters &= !late;
            }
            let mut num_filters = 0;
            for f in [
                PNG_FILTER_SUB,
                PNG_FILTER_UP,
                PNG_FILTER_AVG,
                PNG_FILTER_PAETH,
            ] {
                if filters & u32::from(f) != 0 {
                    num_filters += 1;
                }
            }
            let buf_size =
                crate::structs::rowbytes(self.w.usr_channels * self.w.usr_bit_depth, self.width)
                    + 1;
            if self.w.try_row.is_empty() {
                self.w.try_row = vec![0; buf_size];
            }
            if num_filters > 1 && self.w.tst_row.is_empty() {
                self.w.tst_row = vec![0; buf_size];
            }
        }
        self.w.do_filter = filters as u8;
        Ok(())
    }

    /// Port of `png_set_compression_level` (pngwrite.c#L1216-L1224).
    #[doc(alias = "png_set_compression_level")]
    pub fn set_compression_level(&mut self, level: i32) {
        self.w.zlib_level = level;
    }

    /// Port of `png_set_compression_mem_level` (pngwrite.c#L1227-L1235).
    #[doc(alias = "png_set_compression_mem_level")]
    pub fn set_compression_mem_level(&mut self, mem_level: i32) {
        self.w.zlib_mem_level = mem_level;
    }

    /// Port of `png_set_compression_strategy` (pngwrite.c#L1238-L1250).
    #[doc(alias = "png_set_compression_strategy")]
    pub fn set_compression_strategy(&mut self, strategy: i32) {
        self.flags |= flag::ZLIB_CUSTOM_STRATEGY;
        self.w.zlib_strategy = strategy;
    }

    /// Port of `png_set_compression_window_bits` (pngwrite.c#L1255-L1281).
    #[doc(alias = "png_set_compression_window_bits")]
    pub fn set_compression_window_bits(&mut self, window_bits: i32) {
        let mut window_bits = window_bits;
        if window_bits > 15 {
            self.warning("Only compression windows <= 32k supported by PNG");
            window_bits = 15;
        } else if window_bits < 8 {
            self.warning("Only compression windows >= 256 supported by PNG");
            window_bits = 8;
        }
        self.w.zlib_window_bits = window_bits;
    }

    /// Port of `png_set_compression_method` (pngwrite.c#L1284-L1300).
    #[doc(alias = "png_set_compression_method")]
    pub fn set_compression_method(&mut self, method: i32) {
        if method != 8 {
            self.warning("Only compression method 8 is supported by PNG");
        }
        self.w.zlib_method = method;
    }

    /// Port of `png_set_swap` (pngtrans.c#L34-L44): swap the bytes of 16-bit samples. Only the
    /// write path uses it here, and it depends on the bit depth set by `png_write_info`.
    #[doc(alias = "png_set_swap")]
    pub fn set_swap(&mut self) {
        if self.bit_depth == 16 {
            self.transformations |= crate::wtran::PNG_SWAP_BYTES;
        }
    }

    /// Port of `png_set_filler` on the write side (pngtrans.c#L150-L230): strips the filler
    /// channel of an RGB or gray (8-bit or more) row, as the application's data has it.
    /// `filler_after` is `PNG_FILLER_AFTER` (1) or `PNG_FILLER_BEFORE` (0).
    #[doc(alias = "png_set_filler")]
    pub fn set_filler(&mut self, filler: u32, filler_after: bool) {
        let _ = filler;
        match self.color_type {
            2 => self.w.usr_channels = 4,
            0 if self.bit_depth >= 8 => self.w.usr_channels = 2,
            0 => {
                self.warning("png_set_filler is invalid for low bit depth gray output");
                return;
            }
            _ => {
                self.warning("png_set_filler: inappropriate color type");
                return;
            }
        }
        self.transformations |= crate::wtran::PNG_FILLER;
        if filler_after {
            self.flags |= flag::FILLER_AFTER;
        } else {
            self.flags &= !flag::FILLER_AFTER;
        }
    }

    /// Port of `png_write_info_before_PLTE` (pngwrite.c#L83-L220).
    #[doc(alias = "png_write_info_before_PLTE")]
    pub(crate) fn write_info_before_plte(&mut self, info: &PngInfo) -> PngResult<()> {
        if self.mode & mode::WROTE_INFO_BEFORE_PLTE == 0 {
            self.write_sig()?;
            self.write_ihdr(
                info.width,
                info.height,
                info.bit_depth,
                info.color_type,
                info.compression_type,
                info.filter_type,
                info.interlace_type,
            )?;
            self.write_unknown_chunks(info, mode::HAVE_IHDR)?;
            if info.valid & info::SBIT != 0 {
                self.write_sbit(info.sig_bit, info.color_type)?;
            }
            for (valid, name) in [
                (info::CLLI, "cLLI"),
                (info::MDCV, "mDCV"),
                (info::CICP, "cICP"),
            ] {
                if info.valid & valid != 0 {
                    return Err(self.error(&format!("writing {name} is not ported")));
                }
            }
            if info.valid & info::ICCP != 0 {
                self.write_iccp(&info.iccp_name, &info.iccp_profile)?;
            }
            if info.valid & info::SRGB != 0 {
                self.write_srgb(i32::from(info.srgb_intent))?;
            }
            for (valid, name) in [(info::GAMA, "gAMA"), (info::CHRM, "cHRM")] {
                if info.valid & valid != 0 {
                    return Err(self.error(&format!("writing {name} is not ported")));
                }
            }
            self.mode |= mode::WROTE_INFO_BEFORE_PLTE;
        }
        Ok(())
    }

    /// Port of `png_write_info` (pngwrite.c#L223-L389). The text chunks are marked as written,
    /// so `png_write_end` does not write them again.
    #[doc(alias = "png_write_info")]
    pub fn write_info(&mut self, info: &PngInfo) -> PngResult<()> {
        self.write_info_before_plte(info)?;
        if info.valid & info::PLTE != 0 {
            self.write_plte(&info.palette)?;
        } else if info.color_type == 3 {
            return Err(self.error("Valid palette required for paletted images"));
        }
        for (valid, name) in [
            (info::TRNS, "tRNS"),
            (info::BKGD, "bKGD"),
            (info::HIST, "hIST"),
            (info::OFFS, "oFFs"),
            (info::PCAL, "pCAL"),
            (info::SCAL, "sCAL"),
            (info::PHYS, "pHYs"),
            (info::SPLT, "sPLT"),
        ] {
            if info.valid & valid != 0 {
                return Err(self.error(&format!("writing {name} is not ported")));
            }
        }
        if info.valid & info::TIME != 0 {
            return Err(self.error("writing tIME is not ported"));
        }
        self.write_texts()?;
        self.write_unknown_chunks(info, mode::HAVE_PLTE)?;
        Ok(())
    }

    /// Port of the `png_write_info` and `png_write_end` text loops (pngwrite.c#L255-L290 and
    /// #L430-L465): texts not yet written are written as `tEXt` or `zTXt`, and are then marked.
    fn write_texts(&mut self) -> PngResult<()> {
        let count = self.texts.len();
        for i in 0..count {
            if self.texts[i].written {
                continue;
            }
            let key = self.texts[i].key.clone();
            let text = self.texts[i].text.clone();
            match self.texts[i].compression {
                crate::set::TextCompression::None => self.write_text_chunk(&key, &text)?,
                crate::set::TextCompression::Ztxt => self.write_ztxt(&key, &text)?,
            }
            self.texts[i].written = true;
        }
        Ok(())
    }

    /// Port of `png_write_end` (pngwrite.c#L392-L480).
    #[doc(alias = "png_write_end")]
    pub fn write_end(&mut self, info: &PngInfo) -> PngResult<()> {
        if self.mode & mode::HAVE_IDAT == 0 {
            return Err(self.error("No IDATs written into file"));
        }
        if info.valid & info::TIME != 0 && self.mode & mode::WROTE_TIME == 0 {
            return Err(self.error("writing tIME is not ported"));
        }
        self.write_texts()?;
        if info.valid & info::EXIF != 0 && self.mode & mode::WROTE_EXIF == 0 {
            return Err(self.error("writing eXIf is not ported"));
        }
        self.write_unknown_chunks(info, mode::AFTER_IDAT)?;
        self.mode |= mode::AFTER_IDAT;
        self.write_iend()?;
        Ok(())
    }

    /// Port of `write_unknown_chunks` (pngwrite.c#L23-L80).
    fn write_unknown_chunks(&mut self, info: &PngInfo, where_: u32) -> PngResult<()> {
        for up in &info.unknown_chunks {
            if u32::from(up.location) & where_ == 0 {
                continue;
            }
            let name = u32::from_be_bytes([up.name[0], up.name[1], up.name[2], up.name[3]]);
            let keep = self.chunk_unknown_handling_for_name(name);
            if keep != crate::png::PNG_HANDLE_CHUNK_NEVER
                && ((up.name[3] & 0x20) != 0
                    || keep == crate::png::PNG_HANDLE_CHUNK_ALWAYS
                    || (keep == crate::png::PNG_HANDLE_CHUNK_AS_DEFAULT
                        && self.unknown_default == crate::png::PNG_HANDLE_CHUNK_ALWAYS))
            {
                if up.data.is_empty() {
                    self.warning("Writing zero-length unknown chunk");
                }
                self.write_chunk(&up.name[..4], &up.data)?;
            }
        }
        Ok(())
    }

    /// Port of `png_write_rows` (pngwrite.c#L627-L645).
    #[doc(alias = "png_write_rows")]
    pub fn write_rows(&mut self, rows: &[&[u8]]) -> PngResult<()> {
        for row in rows {
            self.write_row(row)?;
        }
        Ok(())
    }

    /// Port of `png_write_row` (pngwrite.c#L746-L953), without the interlaced passes (Skia writes
    /// `PNG_INTERLACE_NONE` only).
    #[doc(alias = "png_write_row")]
    pub fn write_row(&mut self, row: &[u8]) -> PngResult<()> {
        if self.w.row_number == 0 && self.w.pass == 0 {
            if self.mode & mode::WROTE_INFO_BEFORE_PLTE == 0 {
                return Err(self.error("png_write_info was never called before png_write_row"));
            }
            self.write_start_row();
        }
        let mut row_info = RowInfo {
            color_type: self.color_type,
            width: self.w.usr_width,
            channels: self.w.usr_channels,
            bit_depth: self.w.usr_bit_depth,
            pixel_depth: 0,
            rowbytes: 0,
        };
        row_info.pixel_depth = row_info.bit_depth * row_info.channels;
        row_info.rowbytes = crate::structs::rowbytes(row_info.pixel_depth, row_info.width);
        if row.len() < row_info.rowbytes {
            return Err(self.error("png_write_row: row is shorter than rowbytes"));
        }
        let rb = row_info.rowbytes;
        self.w.row_buf[1..=rb].copy_from_slice(&row[..rb]);

        if self.transformations != 0 {
            let mut data = std::mem::take(&mut self.w.row_buf);
            self.do_write_transformations(&mut row_info, &mut data[1..]);
            self.w.row_buf = data;
        }

        // At this point the row_info pixel depth must match the 'transformed' depth, which is
        // also the output depth.
        if row_info.pixel_depth != self.pixel_depth
            || row_info.pixel_depth != self.w.transformed_pixel_depth
        {
            return Err(self.error("internal write transform logic error"));
        }
        self.write_find_filter(row_info.rowbytes, row_info.pixel_depth)
    }

    /// Port of `png_write_start_row` (pngwutil.c#L1929-L2026). The interlaced branch is not
    /// ported (interlacing is rejected by `png_write_IHDR`).
    #[doc(alias = "png_write_start_row")]
    pub(crate) fn write_start_row(&mut self) {
        let usr_pixel_depth = self.w.usr_channels * self.w.usr_bit_depth;
        let buf_size = crate::structs::rowbytes(usr_pixel_depth, self.width) + 1;
        self.w.transformed_pixel_depth = self.pixel_depth;
        self.w.maximum_pixel_depth = usr_pixel_depth;
        self.w.row_buf = vec![0; buf_size];
        // row_buf[0] is the filter byte, PNG_FILTER_VALUE_NONE.
        let mut filters = u32::from(self.w.do_filter);
        if self.height == 1 {
            filters &= 0xff
                & !(u32::from(PNG_FILTER_UP)
                    | u32::from(PNG_FILTER_AVG)
                    | u32::from(PNG_FILTER_PAETH));
        }
        if self.width == 1 {
            filters &= 0xff
                & !(u32::from(PNG_FILTER_SUB)
                    | u32::from(PNG_FILTER_AVG)
                    | u32::from(PNG_FILTER_PAETH));
        }
        if filters == 0 {
            filters = u32::from(PNG_FILTER_NONE);
        }
        self.w.do_filter = filters as u8;

        let multi = u32::from(PNG_FILTER_SUB)
            | u32::from(PNG_FILTER_UP)
            | u32::from(PNG_FILTER_AVG)
            | u32::from(PNG_FILTER_PAETH);
        if filters & multi != 0 && self.w.try_row.is_empty() {
            self.w.try_row = vec![0; buf_size];
            let mut num_filters = 0;
            for f in [
                PNG_FILTER_SUB,
                PNG_FILTER_UP,
                PNG_FILTER_AVG,
                PNG_FILTER_PAETH,
            ] {
                if filters & u32::from(f) != 0 {
                    num_filters += 1;
                }
            }
            if num_filters > 1 {
                self.w.tst_row = vec![0; buf_size];
            }
        }
        // We only need to keep the previous row if we are using one of the following filters.
        if filters
            & (u32::from(PNG_FILTER_AVG) | u32::from(PNG_FILTER_UP) | u32::from(PNG_FILTER_PAETH))
            != 0
        {
            self.w.prev_row = vec![0; buf_size];
        }
        self.w.num_rows = self.height;
        self.w.usr_width = self.width;
    }

    /// Port of `png_write_finish_row` (pngwutil.c#L2028-L2100), without the interlaced passes.
    #[doc(alias = "png_write_finish_row")]
    pub(crate) fn write_finish_row(&mut self) -> PngResult<()> {
        self.w.row_number += 1;
        if self.w.row_number < self.w.num_rows {
            return Ok(());
        }
        // If we get here, we've just written the last row, so we need to flush the compressor.
        self.compress_idat(&[], Flush::Finish)
    }

    /// Port of `png_write_IEND` (pngwutil.c#L1074-L1082).
    pub(crate) fn write_iend(&mut self) -> PngResult<()> {
        self.write_complete_chunk(PNG_IEND, &[])?;
        self.mode |= mode::HAVE_IEND;
        Ok(())
    }

    /// Port of `png_write_sig` (pngwutil.c#L68-L84).
    pub(crate) fn write_sig(&mut self) -> PngResult<()> {
        const SIG: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
        let start = self.sig_bytes;
        self.write_data(&SIG[start..])?;
        if self.sig_bytes < 3 {
            self.mode |= mode::HAVE_PNG_SIGNATURE;
        }
        Ok(())
    }

    /// Port of `png_write_data` (pngwio.c#L75-L100): hands a piece of output to the application.
    pub(crate) fn write_data(&mut self, data: &[u8]) -> PngResult<()> {
        if let Some(f) = self.w.write_fn.as_mut() {
            if f(data) {
                return Ok(());
            }
            return Err(self.error("sk_write_fn cannot write to stream"));
        }
        Err(self.error("Call to NULL write function"))
    }
}

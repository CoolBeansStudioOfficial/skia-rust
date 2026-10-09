// Copyright (C) 2004, 2006-2025 Glenn Randers-Pehrson and the libpng contributors.
// Use of this source code is governed by the libpng licence (libpng-2.0) in the LICENSE file.
// Port of: pngstruct.h and pnginfo.h (libpng 1.6.56, skia.googlesource.com/third_party/libpng@d5515b5b),
// the fields the read path uses. Field names follow the C names.

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
use skia_rust_zlib::Inflate;

use crate::png::ProgressiveHandler;
use crate::set::TextChunk;

/// Port of `PNG_FLAG_*` (pngstruct.h).
pub(crate) mod flag {
    pub const ZSTREAM_INITIALIZED: u32 = 0x0002;
    pub const ZSTREAM_ENDED: u32 = 0x0008;
    pub const ROW_INIT: u32 = 0x0040;
    pub const CRC_ANCILLARY_USE: u32 = 0x0100;
    pub const CRC_ANCILLARY_NOWARN: u32 = 0x0200;
    pub const CRC_CRITICAL_USE: u32 = 0x0400;
    pub const CRC_CRITICAL_IGNORE: u32 = 0x0800;
    pub const BENIGN_ERRORS_WARN: u32 = 0x0010_0000;
    pub const APP_WARNINGS_WARN: u32 = 0x0020_0000;
    pub const CRC_ANCILLARY_MASK: u32 = CRC_ANCILLARY_USE | CRC_ANCILLARY_NOWARN;
}

/// Port of `PNG_HAVE_*`, `PNG_AFTER_IDAT` and `PNG_IS_READ_STRUCT` (pngpriv.h).
pub(crate) mod mode {
    pub const HAVE_IHDR: u32 = 0x01;
    pub const HAVE_PLTE: u32 = 0x02;
    pub const HAVE_IDAT: u32 = 0x04;
    pub const AFTER_IDAT: u32 = 0x08;
    pub const HAVE_IEND: u32 = 0x10;
    pub const HAVE_CHUNK_HEADER: u32 = 0x100;
    pub const HAVE_PNG_SIGNATURE: u32 = 0x1000;
    pub const HAVE_CHUNK_AFTER_IDAT: u32 = 0x2000;
    pub const IS_READ_STRUCT: u32 = 0x8000;
}

/// Port of `PNG_INFO_*` (png.h): the `valid` bits of `png_info`.
pub mod info {
    pub const GAMA: u32 = 0x0001;
    pub const SBIT: u32 = 0x0002;
    pub const CHRM: u32 = 0x0004;
    pub const PLTE: u32 = 0x0008;
    pub const TRNS: u32 = 0x0010;
    pub const BKGD: u32 = 0x0020;
    pub const HIST: u32 = 0x0040;
    pub const PHYS: u32 = 0x0080;
    pub const OFFS: u32 = 0x0100;
    pub const TIME: u32 = 0x0200;
    pub const PCAL: u32 = 0x0400;
    pub const SRGB: u32 = 0x0800;
    pub const ICCP: u32 = 0x1000;
    pub const SPLT: u32 = 0x2000;
    pub const SCAL: u32 = 0x4000;
    pub const IDAT: u32 = 0x8000;
    pub const EXIF: u32 = 0x0001_0000;
    pub const CICP: u32 = 0x0002_0000;
    pub const CLLI: u32 = 0x0004_0000;
    pub const MDCV: u32 = 0x0008_0000;
}

/// Port of `png_color` (png.h): one palette entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PngColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

/// Port of `png_color_16`: a 16-bit colour, used by `tRNS` and `bKGD`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PngColor16 {
    pub index: u8,
    pub red: u16,
    pub green: u16,
    pub blue: u16,
    pub gray: u16,
}

/// Port of `png_color_8`: significant bits, used by `sBIT`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PngColor8 {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub gray: u8,
    pub alpha: u8,
}

/// Port of `png_unknown_chunk`: a chunk libpng stores verbatim (`png_handle_unknown`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownChunk {
    /// The four name bytes, and a terminating zero.
    pub name: [u8; 5],
    pub data: Vec<u8>,
    /// Port of `location`: the `mode` bits when the chunk was read.
    pub location: u8,
}

/// Port of `png_info` (pnginfo.h): the header information filled in while reading.
#[derive(Debug, Default)]
pub struct PngInfo {
    /// Port of `signature`: the eight bytes the signature check read.
    pub signature: [u8; 8],
    /// Port of `valid`: the `PNG_INFO_*` bits of chunks that were read.
    pub valid: u32,
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,
    pub color_type: u8,
    pub compression_type: u8,
    pub filter_type: u8,
    pub interlace_type: u8,
    pub channels: u8,
    pub pixel_depth: u8,
    pub rowbytes: usize,
    /// Port of `palette` (`num_palette` entries).
    pub palette: Vec<PngColor>,
    /// Port of `trans_alpha` (`num_trans` values, padded to 256 with 0xff).
    pub trans_alpha: Vec<u8>,
    /// Port of `num_trans`.
    pub num_trans: u16,
    /// Port of `trans_color` (the `tRNS` colour key for gray and RGB images).
    pub trans_color: PngColor16,
    /// Port of `iccp_name`, `iccp_profile` and `iccp_compression` (`iCCP`).
    pub iccp_name: String,
    pub iccp_profile: Vec<u8>,
    pub iccp_compression: u8,
    /// Port of `gamma` (`gAMA`), fixed-point times 100000.
    pub gamma: u32,
    /// Port of `srgb_intent` (`sRGB`).
    pub srgb_intent: u8,
    /// Port of `int_chrm` (`cHRM`): white x, white y, red x, red y, green x, green y, blue x,
    /// blue y, as fixed-point times 100000.
    pub int_chrm: [u32; 8],
    /// Port of `sig_bit` (`sBIT`).
    pub sig_bit: PngColor8,
    /// Port of `unknown_chunks` (kept chunks).
    pub unknown_chunks: Vec<UnknownChunk>,
    /// Port of `eXIf` payload.
    pub exif: Vec<u8>,
    /// Port of `cICP` (colour primaries, transfer, matrix, full range).
    pub cicp: [u8; 4],
    /// Port of `cLLI` (max and average light level).
    pub clli: [u32; 2],
}

/// Port of `png_struct` (pngstruct.h): the reader state, as the read path uses it. The C struct
/// also carries the write side and the simplified API, which are not ported.
#[derive(Default)]
pub struct PngStruct {
    /// Port of `flags`.
    pub(crate) flags: u32,
    /// Port of `mode`.
    pub(crate) mode: u32,
    /// Port of `chunks`: one bit per known chunk type that was read (`png_file_has_chunk`).
    pub(crate) chunks: u32,
    /// Port of `options`: two bits per option (`PNG_OPTION_*`).
    pub(crate) options: u32,
    /// Port of `process_mode` (the `PNG_READ_*_MODE` states of pngpread.c).
    pub(crate) process_mode: u8,
    /// Port of `chunk_name`: the current chunk type as a big-endian u32.
    pub(crate) chunk_name: u32,
    /// Port of `push_length`: the length field of the chunk being read.
    pub(crate) push_length: u32,
    /// Port of `idat_size`: IDAT bytes still to read in this chunk.
    pub(crate) idat_size: u32,
    /// Port of `sig_bytes`: signature bytes checked so far.
    pub(crate) sig_bytes: usize,
    /// Port of `crc`: the CRC of the current chunk.
    pub(crate) crc: u32,

    // Port of the progressive input buffers (`save_buffer*`, `current_buffer*`, `buffer_size`).
    // The valid bytes of `save_buffer` are `save_buffer[save_buffer_ptr..][..save_buffer_size]`,
    // and of `current_buffer` are `current_buffer[current_buffer_ptr..][..current_buffer_size]`.
    pub(crate) save_buffer: Vec<u8>,
    pub(crate) save_buffer_ptr: usize,
    pub(crate) save_buffer_size: usize,
    pub(crate) current_buffer: Vec<u8>,
    pub(crate) current_buffer_ptr: usize,
    pub(crate) current_buffer_size: usize,
    pub(crate) buffer_size: usize,

    /// Port of `zstream`: the inflate state. `None` until `png_inflate_claim` initializes it.
    pub(crate) zstream: Option<Inflate>,
    /// Port of `zstream.avail_out`, with `zstream_next_out` as the offset into `row_buf`.
    pub(crate) zstream_avail_out: usize,
    pub(crate) zstream_next_out: usize,
    /// Port of `zstream.next_in`/`avail_in` for the non-progressive inflate paths: the input not
    /// yet consumed, `zin[zin_pos..]`.
    pub(crate) zin: Vec<u8>,
    pub(crate) zin_pos: usize,
    /// Port of `zstream_start`: the window-size check still to do on the first byte.
    pub(crate) zstream_start: bool,
    /// Port of `zowner`: the chunk name that owns the inflate stream, or 0.
    pub(crate) zowner: u32,
    /// Port of `zstream.msg`: the last message from zlib, cleared when the stream is reset.
    pub(crate) zstream_msg: Option<&'static str>,

    /// Port of `row_buf`: one filter byte plus one row of pixels.
    pub(crate) row_buf: Vec<u8>,
    /// Port of `prev_row`: the previous unfiltered row, with its filter byte.
    pub(crate) prev_row: Vec<u8>,
    /// Port of `row_number`, `num_rows`, `pass`, `iwidth`.
    pub(crate) row_number: u32,
    pub(crate) num_rows: u32,
    pub(crate) pass: u32,
    pub(crate) iwidth: u32,
    /// Port of the source header fields (`width`, `height`, `bit_depth`, ...).
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) bit_depth: u8,
    pub(crate) color_type: u8,
    pub(crate) channels: u8,
    pub(crate) pixel_depth: u8,
    pub(crate) rowbytes: usize,
    pub(crate) interlaced: u8,
    pub(crate) filter_type: u8,
    pub(crate) compression_type: u8,
    /// Port of `info_rowbytes`: the rowbytes `png_read_transform_info` computed.
    pub(crate) info_rowbytes: usize,
    /// Port of `transformations` (the `PNG_*` transform bits).
    pub(crate) transformations: u32,
    /// Port of `transformed_pixel_depth` and `maximum_pixel_depth`.
    pub(crate) transformed_pixel_depth: u8,
    pub(crate) maximum_pixel_depth: u8,

    /// Port of `info_fn`, `row_fn` and `end_fn` (with the user data in the handler).
    pub(crate) progressive: Option<Box<dyn ProgressiveHandler>>,

    /// Port of `read_user_chunk_fn` (`png_set_read_user_chunk_fn`).
    pub(crate) user_chunk: Option<Box<dyn crate::png::UserChunkReader>>,
    /// Port of `chunk_list` and `num_chunk_list`: the keep rules from `png_set_keep_unknown_chunks`.
    pub(crate) chunk_list: Vec<(Vec<u8>, u8)>,
    /// Port of `unknown_default`.
    pub(crate) unknown_default: u8,

    /// Port of `palette` and `num_palette`, for the checks that read them (`tRNS`, `hIST`, `bKGD`).
    pub(crate) palette: Vec<PngColor>,
    pub(crate) num_palette: u32,
    /// Port of `trans_alpha`, `num_trans` and `trans_color` (the reader's copy, used by the
    /// transforms).
    pub(crate) trans_alpha: Vec<u8>,
    pub(crate) num_trans: u16,
    pub(crate) trans_color: PngColor16,
    /// Port of `background` (`bKGD`), kept for the `bKGD` checks.
    pub(crate) background: PngColor16,
    /// Port of `sig_bit` (`sBIT`) as the reader keeps it.
    pub(crate) sig_bit: PngColor8,
    /// Port of `chromaticities` and `chunk_gamma`, used by `cHRM`, `gAMA` and `sRGB` handling.
    pub(crate) chromaticities: [i32; 8],
    pub(crate) chunk_gamma: u32,
    /// Port of `hist`, `phys`, `offs`, `time`, `texts`: values the read path stores.
    pub(crate) hist: Vec<u16>,
    pub(crate) phys: (u32, u32, u8),
    pub(crate) offs: (i32, i32, u8),
    pub(crate) time: (u16, u8, u8, u8, u8, u8),
    pub(crate) texts: Vec<TextChunk>,

    /// Port of `user_width_max`, `user_height_max`, `user_chunk_malloc_max` and
    /// `user_chunk_cache_max` (`png_set_user_limits`, `png_set_chunk_cache_max`).
    pub(crate) user_width_max: u32,
    pub(crate) user_height_max: u32,
    pub(crate) user_chunk_malloc_max: usize,
    pub(crate) user_chunk_cache_max: u32,

    /// Warnings libpng would have sent to the application's warning function, in order.
    pub warnings: Vec<String>,
}

impl std::fmt::Debug for PngStruct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PngStruct")
            .field("mode", &self.mode)
            .field("flags", &self.flags)
            .field("process_mode", &self.process_mode)
            .field("row_number", &self.row_number)
            .finish_non_exhaustive()
    }
}

/// Port of `PNG_ROWBYTES(pixel_bits, width)` (pngpriv.h).
#[must_use]
pub(crate) fn rowbytes(pixel_bits: u8, width: u32) -> usize {
    if pixel_bits >= 8 {
        (width as usize) * (usize::from(pixel_bits) >> 3)
    } else {
        ((width as usize) * usize::from(pixel_bits) + 7) >> 3
    }
}

/// Port of `PNG_CHUNK_FROM_STRING`: four bytes in order, as a big-endian u32.
#[must_use]
pub(crate) fn chunk_from_bytes(b: &[u8]) -> u32 {
    (u32::from(b[0]) << 24) | (u32::from(b[1]) << 16) | (u32::from(b[2]) << 8) | u32::from(b[3])
}

/// Port of `PNG_CHUNK_ANCILLARY(c)` (pngpriv.h): 1 for an ancillary chunk, else 0.
#[must_use]
pub(crate) fn chunk_ancillary(name: u32) -> u32 {
    1 & (name >> 29)
}

/// Port of `PNG_CHUNK_CRITICAL(c)` (pngpriv.h): the opposite of ancillary.
#[must_use]
pub(crate) fn chunk_critical(name: u32) -> bool {
    chunk_ancillary(name) == 0
}

/// Port of `PNG_STRING_FROM_CHUNK`: the four-character name of a chunk.
#[must_use]
pub(crate) fn chunk_string(name: u32) -> String {
    name.to_be_bytes().iter().map(|&b| char::from(b)).collect()
}

/// Chunk-type constants (`png_IDAT` etc., pngpriv.h).
pub(crate) const PNG_IDAT: u32 = chunk_from_bytes_const(*b"IDAT");
pub(crate) const PNG_IHDR: u32 = chunk_from_bytes_const(*b"IHDR");
pub(crate) const PNG_IEND: u32 = chunk_from_bytes_const(*b"IEND");
pub(crate) const PNG_PLTE: u32 = chunk_from_bytes_const(*b"PLTE");

const fn chunk_from_bytes_const(b: [u8; 4]) -> u32 {
    ((b[0] as u32) << 24) | ((b[1] as u32) << 16) | ((b[2] as u32) << 8) | (b[3] as u32)
}

/// Port of the text-chunk compression values (`PNG_TEXT_COMPRESSION_*`), re-exported where the
/// chunk handlers use them.
pub(crate) use crate::set::TextCompression;

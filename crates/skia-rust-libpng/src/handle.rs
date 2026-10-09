// Copyright (C) 1998-2025 Glenn Randers-Pehrson and the libpng contributors.
// Use of this source code is governed by the libpng licence (libpng-2.0) in the LICENSE file.
// Port of: pngrutil.c#L1-L2703 (libpng 1.6.56, skia.googlesource.com/third_party/libpng@d5515b5b):
// the chunk dispatch (`png_handle_chunk`, the `read_chunks` table), unknown chunks
// (`png_handle_unknown`) and the ancillary chunk handlers.

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
use crate::rutil::{HandleResult, PNG_INFLATE_BUF_SIZE, get_int_32, get_uint_16, get_uint_32};
use crate::structs::{PngInfo, PngStruct, UnknownChunk, chunk_from_bytes, mode};

/// Port of `PNG_HANDLE_CHUNK_*` values used by `png_handle_unknown`.
const KEEP_NEVER: u8 = crate::png::PNG_HANDLE_CHUNK_NEVER;
const KEEP_IF_SAFE: u8 = crate::png::PNG_HANDLE_CHUNK_IF_SAFE;
const KEEP_ALWAYS: u8 = crate::png::PNG_HANDLE_CHUNK_ALWAYS;
const KEEP_AS_DEFAULT: u8 = crate::png::PNG_HANDLE_CHUNK_AS_DEFAULT;

/// Port of `PNG_COMPRESSION_TYPE_BASE` (png.h).
const PNG_COMPRESSION_TYPE_BASE: u8 = 0;
/// Port of `LZ77Min` (pngrutil.c#L5).
const LZ77_MIN: u32 = 2 + 5 + 4;
/// Port of `PNG_MAX_PALETTE_LENGTH` (png.h).
const PNG_MAX_PALETTE_LENGTH: u32 = 256;

/// Port of the `NoCheck`/`Limit` markers of the `read_chunks` table (pngrutil.c#L1648-L1655).
#[derive(Clone, Copy, PartialEq, Eq)]
enum MaxLength {
    /// `NoCheck`: no maximum.
    NoCheck,
    /// `Limit`: at most `png_chunk_max`.
    Limit,
    /// A fixed maximum.
    Max(u32),
}

/// Port of one `read_chunks[]` entry: the length limits, the required chunk-order mode bits and
/// whether the chunk may repeat (`CD*` macros in pngrutil.c#L1649-L1686).
#[derive(Clone, Copy)]
struct ChunkRule {
    max_length: MaxLength,
    min_length: u32,
    pos_before: u32,
    pos_after: u32,
    multiple: bool,
    kind: Handler,
}

/// Which handler a chunk uses. `None` for the chunks Skia's build does not read (`acTL`,
/// `fcTL`, `fdAT`), which go to `png_handle_unknown`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Handler {
    Ihdr,
    Plte,

    Iend,
    Bkgd,
    Chrm,
    Cicp,
    Clli,
    Exif,
    Gama,
    Hist,
    Iccp,
    Itxt,
    Mdcv,
    Offs,
    Pcal,
    Phys,
    Sbit,
    Scal,
    Splt,
    Srgb,
    Text,
    Time,
    Trns,
    Ztxt,
}

/// Port of the `PNG_HAVE_*` and `PNG_AFTER_IDAT` masks used by the `CD*` rules.
const H_IHDR: u32 = mode::HAVE_IHDR;
const H_PLTE: u32 = mode::HAVE_PLTE;
const H_IDAT: u32 = mode::HAVE_IDAT;
const H_COL: u32 = mode::HAVE_PLTE | mode::HAVE_IDAT;
const A_IDAT: u32 = mode::AFTER_IDAT;

/// Port of `png_index` (pngpriv.h) for the known chunks, in `PNG_KNOWN_CHUNKS` order.
#[must_use]
fn chunk_index(name: u32) -> Option<usize> {
    // The order of PNG_KNOWN_CHUNKS (pngpriv.h#L968-L995). Indices are bit positions in `chunks`.
    const NAMES: [&[u8; 4]; 28] = [
        b"IHDR", b"PLTE", b"IDAT", b"IEND", b"acTL", b"bKGD", b"cHRM", b"cICP", b"cLLI", b"eXIf",
        b"fcTL", b"fdAT", b"gAMA", b"hIST", b"iCCP", b"iTXt", b"mDCV", b"oFFs", b"pCAL", b"pHYs",
        b"sBIT", b"sCAL", b"sPLT", b"sRGB", b"tEXt", b"tIME", b"tRNS", b"zTXt",
    ];
    NAMES.iter().position(|n| chunk_from_bytes(*n) == name)
}

/// Port of the `read_chunks[]` table (pngrutil.c#L1649-L1686), indexed by [`chunk_index`].
fn chunk_rule(index: usize) -> Option<ChunkRule> {
    use Handler::{
        Bkgd, Chrm, Cicp, Clli, Exif, Gama, Hist, Iccp, Iend, Ihdr, Itxt, Mdcv, Offs, Pcal, Phys,
        Plte, Sbit, Scal, Splt, Srgb, Text, Time, Trns, Ztxt,
    };
    use MaxLength::{Limit, Max, NoCheck};
    let r = |max_length, min_length, pos_before, pos_after, multiple, kind| ChunkRule {
        max_length,
        min_length,
        pos_before,
        pos_after,
        multiple,
        kind,
    };
    let lkmin = 3 + LZ77_MIN;
    Some(match index {
        0 => r(Max(13), 13, H_IHDR, 0, false, Ihdr),
        1 => r(NoCheck, 0, 0, H_IHDR, true, Plte),

        3 => r(NoCheck, 0, 0, A_IDAT, false, Iend),
        5 => r(Max(6), 1, H_IDAT, H_IHDR, false, Bkgd),
        6 => r(Max(32), 32, H_COL, H_IHDR, false, Chrm),
        7 => r(Max(4), 4, H_COL, H_IHDR, false, Cicp),
        8 => r(Max(8), 8, H_COL, H_IHDR, false, Clli),
        9 => r(Limit, 4, 0, H_IHDR, false, Exif),
        12 => r(Max(4), 4, H_COL, H_IHDR, false, Gama),
        13 => r(Max(1024), 0, H_PLTE, H_IHDR, false, Hist),
        14 => r(NoCheck, lkmin, H_COL, H_IHDR, false, Iccp),
        15 => r(NoCheck, 6, 0, H_IHDR, true, Itxt),
        16 => r(Max(24), 24, H_COL, H_IHDR, false, Mdcv),
        17 => r(Max(9), 9, H_IDAT, H_IHDR, false, Offs),
        18 => r(NoCheck, 14, H_IDAT, H_IHDR, false, Pcal),
        19 => r(Max(9), 9, H_IDAT, H_IHDR, false, Phys),
        20 => r(Max(4), 1, H_COL, H_IHDR, false, Sbit),
        21 => r(Limit, 4, H_IDAT, H_IHDR, false, Scal),
        22 => r(NoCheck, 3, H_IDAT, H_IHDR, true, Splt),
        23 => r(Max(1), 1, H_COL, H_IHDR, false, Srgb),
        24 => r(NoCheck, 2, 0, H_IHDR, true, Text),
        25 => r(Max(7), 7, 0, H_IHDR, false, Time),
        26 => r(Max(256), 0, H_IDAT, H_IHDR, false, Trns),
        27 => r(Limit, lkmin, 0, H_IHDR, true, Ztxt),
        _ => return None,
    })
}

impl PngStruct {
    /// Port of `png_chunk_index_from_name` followed by the table lookup in `png_handle_chunk`.
    /// The `multiple` flag and the `chunks` bitmask follow `png_file_has_chunk` (pngstruct.h).
    fn file_has_chunk(&self, index: usize) -> bool {
        self.chunks & (1 << index) != 0
    }

    /// Port of `png_file_add_chunk` (pngstruct.h#L118).
    fn file_add_chunk(&mut self, index: usize) {
        self.chunks |= 1 << index;
    }

    /// Port of `png_has_chunk` (pngstruct.h#L193). The bit is set when the chunk has been read.
    #[must_use]
    pub(crate) fn has_chunk(&self, name: &str) -> bool {
        chunk_index(chunk_from_bytes(name.as_bytes())).is_some_and(|i| self.file_has_chunk(i))
    }

    /// Port of `png_handle_chunk` (pngrutil.c#L1769-L1832).
    #[doc(alias = "png_handle_chunk")]
    pub(crate) fn handle_chunk(
        &mut self,
        info: &mut PngInfo,
        length: u32,
    ) -> PngResult<HandleResult> {
        let chunk_name = self.chunk_name;
        let index = chunk_index(chunk_name);
        let rule = index.and_then(chunk_rule);
        let mut handled;
        let mut errmsg: Option<&'static str> = None;
        match (index, rule) {
            (None, _) | (Some(_), None) => {
                handled = self.handle_unknown(info, length, KEEP_AS_DEFAULT)?;
            }
            (Some(idx), Some(rule)) => {
                if idx != 0 && self.mode & H_IHDR == 0 {
                    return Err(self.chunk_error("missing IHDR"));
                }
                handled = HandleResult::Error;
                if (self.mode & rule.pos_before) != 0
                    || (self.mode & rule.pos_after) != rule.pos_after
                {
                    errmsg = Some("out of place");
                } else if !rule.multiple && self.file_has_chunk(idx) {
                    errmsg = Some("duplicate");
                } else if length < rule.min_length {
                    errmsg = Some("too short");
                } else {
                    let meets = match rule.max_length {
                        MaxLength::Limit => {
                            if (length as usize) <= self.user_chunk_malloc_max {
                                true
                            } else {
                                errmsg = Some("length exceeds libpng limit");
                                false
                            }
                        }
                        MaxLength::Max(m) => {
                            if length <= m {
                                true
                            } else {
                                errmsg = Some("too long");
                                false
                            }
                        }
                        MaxLength::NoCheck => true,
                    };
                    if meets {
                        handled = self.dispatch_handler(rule.kind, info, length)?;
                    }
                }
                if let Some(msg) = errmsg {
                    if crate::structs::chunk_critical(chunk_name) {
                        return Err(self.chunk_error(msg));
                    }
                    self.crc_finish(length)?;
                    self.chunk_benign_error(msg)?;
                }
                if errmsg.is_none() && handled >= HandleResult::Saved {
                    self.file_add_chunk(idx);
                }
                return Ok(handled);
            }
        }
        Ok(handled)
    }

    /// Calls the handler for `kind`. The C table holds function pointers; this is the same call.
    fn dispatch_handler(
        &mut self,
        kind: Handler,
        info: &mut PngInfo,
        length: u32,
    ) -> PngResult<HandleResult> {
        match kind {
            Handler::Ihdr => self.handle_ihdr(info, length),
            Handler::Plte => self.handle_plte(info, length),

            Handler::Iend => self.handle_iend(info, length),
            Handler::Bkgd => self.handle_bkgd(info, length),
            Handler::Chrm => self.handle_chrm(info, length),
            Handler::Cicp => self.handle_cicp(info, length),
            Handler::Clli => self.handle_clli(info, length),
            Handler::Exif => self.handle_exif(info, length),
            Handler::Gama => self.handle_gama(info, length),
            Handler::Hist => self.handle_hist(info, length),
            Handler::Iccp => self.handle_iccp(info, length),
            Handler::Itxt => self.handle_itxt(info, length),
            Handler::Mdcv => self.handle_mdcv(info, length),
            Handler::Offs => self.handle_offs(info, length),
            Handler::Pcal => self.handle_pcal(info, length),
            Handler::Phys => self.handle_phys(info, length),
            Handler::Sbit => self.handle_sbit(info, length),
            Handler::Scal => self.handle_scal(info, length),
            Handler::Splt => self.handle_splt(info, length),
            Handler::Srgb => self.handle_srgb(info, length),
            Handler::Text => self.handle_text(info, length),
            Handler::Time => self.handle_time(info, length),
            Handler::Trns => self.handle_trns(info, length),
            Handler::Ztxt => self.handle_ztxt(info, length),
        }
    }

    /// Port of `png_cache_unknown_chunk` (pngrutil.c#L1589-L1622). Reads the chunk data into a
    /// buffer and returns `false` when it exceeds the memory limit (after a benign error).
    fn cache_unknown_chunk(&mut self, length: u32) -> PngResult<Option<Vec<u8>>> {
        let limit = self.user_chunk_malloc_max;
        if length as usize > limit {
            self.crc_finish(length)?;
            self.chunk_benign_error("unknown chunk exceeds memory limits")?;
            return Ok(None);
        }
        let mut data = vec![0u8; length as usize];
        if length > 0 {
            self.crc_read(&mut data);
        }
        self.crc_finish(0)?;
        Ok(Some(data))
    }

    /// Port of `png_handle_unknown` (pngrutil.c#L1625-L1752). Returns the handling outcome: `Ok`
    /// (handled by the user), `Discard` (dropped), or `Saved` (stored in `unknown_chunks`).
    #[doc(alias = "png_handle_unknown")]
    pub(crate) fn handle_unknown(
        &mut self,
        info: &mut PngInfo,
        length: u32,
        keep: u8,
    ) -> PngResult<HandleResult> {
        let mut keep = keep;
        let mut handled = HandleResult::Discard;
        let mut data: Option<Vec<u8>> = None;
        if let Some(mut user) = self.user_chunk.take() {
            let cached = self.cache_unknown_chunk(length)?;
            let name = self.chunk_name.to_be_bytes();
            if let Some(bytes) = cached {
                let ret = user.read_chunk(&name, &bytes);
                self.user_chunk = Some(user);
                match ret {
                    r if r < 0 => return Err(self.chunk_error("error in user chunk")),
                    0 => {
                        if keep < KEEP_IF_SAFE {
                            if self.unknown_default < KEEP_IF_SAFE {
                                self.chunk_warning("Saving unknown chunk:");
                            }
                            keep = KEEP_IF_SAFE;
                        }
                    }
                    _ => {
                        handled = HandleResult::Ok;
                        keep = KEEP_NEVER;
                    }
                }
                data = Some(bytes);
            } else {
                self.user_chunk = Some(user);
                keep = KEEP_NEVER;
            }
        } else {
            if keep == KEEP_AS_DEFAULT {
                keep = self.unknown_default;
            }
            if keep == KEEP_ALWAYS
                || (keep == KEEP_IF_SAFE && crate::structs::chunk_ancillary(self.chunk_name) != 0)
            {
                match self.cache_unknown_chunk(length)? {
                    Some(bytes) => data = Some(bytes),
                    None => keep = KEEP_NEVER,
                }
            } else {
                self.crc_finish(length)?;
            }
        }
        if keep == KEEP_ALWAYS
            || (keep == KEEP_IF_SAFE && crate::structs::chunk_ancillary(self.chunk_name) != 0)
        {
            match self.user_chunk_cache_max {
                2 => {
                    self.user_chunk_cache_max = 1;
                    self.chunk_benign_error("no space in chunk cache")?;
                }
                1 => {}
                _ => {
                    if self.user_chunk_cache_max > 0 {
                        self.user_chunk_cache_max -= 1;
                    }
                    self.set_unknown_chunk(info, data.take().unwrap_or_default());
                    handled = HandleResult::Saved;
                }
            }
        }
        if handled < HandleResult::Saved && crate::structs::chunk_critical(self.chunk_name) {
            return Err(self.chunk_error("unhandled critical chunk"));
        }
        Ok(handled)
    }

    /// Port of `png_set_unknown_chunks` for the one chunk `png_handle_unknown` saves.
    fn set_unknown_chunk(&mut self, info: &mut PngInfo, data: Vec<u8>) {
        info.unknown_chunks.push(UnknownChunk {
            name: {
                let b = self.chunk_name.to_be_bytes();
                [b[0], b[1], b[2], b[3], 0]
            },
            data,
            location: self.mode as u8,
        });
    }

    /// Port of `png_handle_tRNS` (pngrutil.c#L927-L992).
    #[doc(alias = "png_handle_tRNS")]
    pub(crate) fn handle_trns(
        &mut self,
        info: &mut PngInfo,
        length: u32,
    ) -> PngResult<HandleResult> {
        let mut readbuf = [0u8; PNG_MAX_PALETTE_LENGTH as usize];
        let mut trans_alpha: Vec<u8>;
        if self.color_type == 0 {
            if length != 2 {
                self.crc_finish(length)?;
                self.chunk_benign_error("invalid")?;
                return Ok(HandleResult::Error);
            }
            let mut buf = [0u8; 2];
            self.crc_read(&mut buf);
            self.num_trans = 1;
            self.trans_color.gray = get_uint_16(&buf);
            // libpng passes its (unused for gray) alpha buffer, `readbuf`, here.
            trans_alpha = vec![0u8];
        } else if self.color_type == 2 {
            if length != 6 {
                self.crc_finish(length)?;
                self.chunk_benign_error("invalid")?;
                return Ok(HandleResult::Error);
            }
            let mut buf = [0u8; 6];
            self.crc_read(&mut buf);
            self.num_trans = 1;
            self.trans_color.red = get_uint_16(&buf[0..2]);
            self.trans_color.green = get_uint_16(&buf[2..4]);
            self.trans_color.blue = get_uint_16(&buf[4..6]);
            trans_alpha = vec![0u8];
        } else if self.color_type == 3 {
            if self.mode & mode::HAVE_PLTE == 0 {
                self.crc_finish(length)?;
                self.chunk_benign_error("out of place")?;
                return Ok(HandleResult::Error);
            }
            if length > self.num_palette || length > PNG_MAX_PALETTE_LENGTH || length == 0 {
                self.crc_finish(length)?;
                self.chunk_benign_error("invalid")?;
                return Ok(HandleResult::Error);
            }
            self.crc_read(&mut readbuf[..length as usize]);
            self.num_trans = length as u16;
            trans_alpha = readbuf[..length as usize].to_vec();
        } else {
            self.crc_finish(length)?;
            self.chunk_benign_error("invalid with alpha channel")?;
            return Ok(HandleResult::Error);
        }
        if self.crc_finish(0)? {
            self.num_trans = 0;
            return Ok(HandleResult::Error);
        }
        let trans_color = self.trans_color;
        self.set_trns(info, &mut trans_alpha, self.num_trans, trans_color);
        Ok(HandleResult::Ok)
    }

    /// Port of `png_handle_bKGD` (pngrutil.c#L994-L1075).
    #[doc(alias = "png_handle_bKGD")]
    pub(crate) fn handle_bkgd(
        &mut self,
        info: &mut PngInfo,
        length: u32,
    ) -> PngResult<HandleResult> {
        let truelen: u32 = if self.color_type == 3 {
            if self.mode & mode::HAVE_PLTE == 0 {
                self.crc_finish(length)?;
                self.chunk_benign_error("out of place")?;
                return Ok(HandleResult::Error);
            }
            1
        } else if self.color_type & 2 != 0 {
            6
        } else {
            2
        };
        if length != truelen {
            self.crc_finish(length)?;
            self.chunk_benign_error("invalid")?;
            return Ok(HandleResult::Error);
        }
        let mut buf = [0u8; 6];
        self.crc_read(&mut buf[..truelen as usize]);
        if self.crc_finish(0)? {
            return Ok(HandleResult::Error);
        }
        let background = if self.color_type == 3 {
            let index = buf[0];
            let (red, green, blue) = if info.palette.is_empty() {
                (0u16, 0u16, 0u16)
            } else {
                if u32::from(index) >= info.palette.len() as u32 {
                    self.chunk_benign_error("invalid index")?;
                    return Ok(HandleResult::Error);
                }
                let p = info.palette[index as usize];
                (u16::from(p.red), u16::from(p.green), u16::from(p.blue))
            };
            crate::structs::PngColor16 {
                index,
                red,
                green,
                blue,
                gray: 0,
            }
        } else if self.color_type & 2 == 0 {
            if self.bit_depth <= 8 && (buf[0] != 0 || u32::from(buf[1]) >= (1u32 << self.bit_depth))
            {
                self.chunk_benign_error("invalid gray level")?;
                return Ok(HandleResult::Error);
            }
            let g = get_uint_16(&buf[0..2]);
            crate::structs::PngColor16 {
                index: 0,
                red: g,
                green: g,
                blue: g,
                gray: g,
            }
        } else {
            if self.bit_depth <= 8 && (buf[0] != 0 || buf[2] != 0 || buf[4] != 0) {
                self.chunk_benign_error("invalid color")?;
                return Ok(HandleResult::Error);
            }
            crate::structs::PngColor16 {
                index: 0,
                red: get_uint_16(&buf[0..2]),
                green: get_uint_16(&buf[2..4]),
                blue: get_uint_16(&buf[4..6]),
                gray: 0,
            }
        };
        self.set_bkgd(info, background);
        Ok(HandleResult::Ok)
    }

    /// Port of `png_handle_cICP` (pngrutil.c#L1077-L1090).
    #[doc(alias = "png_handle_cICP")]
    pub(crate) fn handle_cicp(
        &mut self,
        info: &mut PngInfo,
        _length: u32,
    ) -> PngResult<HandleResult> {
        let mut buf = [0u8; 4];
        self.crc_read(&mut buf);
        if self.crc_finish(0)? {
            return Ok(HandleResult::Error);
        }
        self.set_cicp(info, buf);
        Ok(HandleResult::Ok)
    }

    /// Port of `png_handle_cLLI` (pngrutil.c#L1092-L1103).
    #[doc(alias = "png_handle_cLLI")]
    pub(crate) fn handle_clli(
        &mut self,
        info: &mut PngInfo,
        _length: u32,
    ) -> PngResult<HandleResult> {
        let mut buf = [0u8; 8];
        self.crc_read(&mut buf);
        if self.crc_finish(0)? {
            return Ok(HandleResult::Error);
        }
        self.set_clli_fixed(info, get_uint_32(&buf[0..4]), get_uint_32(&buf[4..8]));
        Ok(HandleResult::Ok)
    }

    /// Port of `png_handle_mDCV` (pngrutil.c#L1105-L1131).
    #[doc(alias = "png_handle_mDCV")]
    pub(crate) fn handle_mdcv(
        &mut self,
        info: &mut PngInfo,
        _length: u32,
    ) -> PngResult<HandleResult> {
        let mut buf = [0u8; 24];
        self.crc_read(&mut buf);
        if self.crc_finish(0)? {
            return Ok(HandleResult::Error);
        }
        // Each chromaticity is a 16-bit value doubled (pngrutil.c#L1113-L1120).
        let c = |i: usize| u32::from(get_uint_16(&buf[i..i + 2])) << 1;
        let chromaticities = [c(12), c(14), c(0), c(2), c(4), c(6), c(8), c(10)];
        self.set_mdcv_fixed(
            info,
            chromaticities,
            get_uint_32(&buf[16..20]),
            get_uint_32(&buf[20..24]),
        );
        self.chromaticities = [
            chromaticities[2] as i32,
            chromaticities[3] as i32,
            chromaticities[4] as i32,
            chromaticities[5] as i32,
            chromaticities[6] as i32,
            chromaticities[7] as i32,
            chromaticities[0] as i32,
            chromaticities[1] as i32,
        ];
        Ok(HandleResult::Ok)
    }

    /// Port of `png_handle_eXIf` (pngrutil.c#L1133-L1157).
    #[doc(alias = "png_handle_eXIf")]
    pub(crate) fn handle_exif(
        &mut self,
        info: &mut PngInfo,
        length: u32,
    ) -> PngResult<HandleResult> {
        let Some(mut buffer) = self.read_buffer_alloc(length) else {
            self.crc_finish(length)?;
            self.chunk_benign_error("out of memory")?;
            return Ok(HandleResult::Error);
        };
        self.crc_read(&mut buffer);
        if self.crc_finish(0)? {
            return Ok(HandleResult::Error);
        }
        let header = get_uint_32(&buffer);
        if header != 0x4949_2A00 && header != 0x4D4D_002A {
            self.chunk_benign_error("invalid")?;
            return Ok(HandleResult::Error);
        }
        self.set_exif_1(info, buffer);
        Ok(HandleResult::Ok)
    }

    /// Port of `png_handle_hIST` (pngrutil.c#L1159-L1183).
    #[doc(alias = "png_handle_hIST")]
    pub(crate) fn handle_hist(
        &mut self,
        info: &mut PngInfo,
        length: u32,
    ) -> PngResult<HandleResult> {
        let num = length / 2;
        if length != num * 2 || num != self.num_palette || num > PNG_MAX_PALETTE_LENGTH {
            self.crc_finish(length)?;
            self.chunk_benign_error("invalid")?;
            return Ok(HandleResult::Error);
        }
        let mut readbuf = Vec::with_capacity(num as usize);
        for _ in 0..num {
            let mut buf = [0u8; 2];
            self.crc_read(&mut buf);
            readbuf.push(get_uint_16(&buf));
        }
        if self.crc_finish(0)? {
            return Ok(HandleResult::Error);
        }
        self.set_hist(info, readbuf);
        Ok(HandleResult::Ok)
    }

    /// Port of `png_handle_pHYs` (pngrutil.c#L1185-L1200).
    #[doc(alias = "png_handle_pHYs")]
    pub(crate) fn handle_phys(
        &mut self,
        info: &mut PngInfo,
        _length: u32,
    ) -> PngResult<HandleResult> {
        let mut buf = [0u8; 9];
        self.crc_read(&mut buf);
        if self.crc_finish(0)? {
            return Ok(HandleResult::Error);
        }
        self.set_phys(
            info,
            get_uint_32(&buf[0..4]),
            get_uint_32(&buf[4..8]),
            buf[8],
        );
        Ok(HandleResult::Ok)
    }

    /// Port of `png_handle_oFFs` (pngrutil.c#L1202-L1217).
    #[doc(alias = "png_handle_oFFs")]
    pub(crate) fn handle_offs(
        &mut self,
        info: &mut PngInfo,
        _length: u32,
    ) -> PngResult<HandleResult> {
        let mut buf = [0u8; 9];
        self.crc_read(&mut buf);
        if self.crc_finish(0)? {
            return Ok(HandleResult::Error);
        }
        self.set_offs(info, get_int_32(&buf[0..4]), get_int_32(&buf[4..8]), buf[8]);
        Ok(HandleResult::Ok)
    }

    /// Port of `png_handle_tIME` (pngrutil.c#L1351-L1370).
    #[doc(alias = "png_handle_tIME")]
    pub(crate) fn handle_time(
        &mut self,
        info: &mut PngInfo,
        _length: u32,
    ) -> PngResult<HandleResult> {
        if self.mode & mode::HAVE_IDAT != 0 {
            self.mode |= mode::AFTER_IDAT;
        }
        let mut buf = [0u8; 7];
        self.crc_read(&mut buf);
        if self.crc_finish(0)? {
            return Ok(HandleResult::Error);
        }
        self.set_time(
            info,
            get_uint_16(&buf[0..2]),
            buf[2],
            buf[3],
            buf[4],
            buf[5],
            buf[6],
        );
        Ok(HandleResult::Ok)
    }

    /// Reads the whole chunk into a buffer, with the `read_buffer` limit. `None` where
    /// `png_read_buffer` returns NULL.
    fn read_chunk_buffer(&mut self, length: u32) -> Option<Vec<u8>> {
        self.read_buffer_alloc(length)
    }

    /// Port of the `user_chunk_cache_max` countdown shared by tEXt, zTXt and iTXt
    /// (pngrutil.c#L1378-L1385). Returns `Some(Ok(()))` to go on, or the result to return.
    pub(crate) fn text_cache_budget(&mut self, length: u32) -> PngResult<Option<HandleResult>> {
        if self.user_chunk_cache_max != 0 {
            if self.user_chunk_cache_max == 1 {
                self.crc_finish(length)?;
                return Ok(Some(HandleResult::Error));
            }
            self.user_chunk_cache_max -= 1;
            if self.user_chunk_cache_max == 1 {
                self.crc_finish(length)?;
                self.chunk_benign_error("no space in chunk cache")?;
                return Ok(Some(HandleResult::Error));
            }
        }
        Ok(None)
    }

    /// Port of `png_handle_tEXt` (pngrutil.c#L1372-L1421). The text is parsed and validated, and
    /// stored in the info struct only as far as `png_set_text_2` needs (keyword and text).
    #[doc(alias = "png_handle_tEXt")]
    pub(crate) fn handle_text(
        &mut self,
        info: &mut PngInfo,
        length: u32,
    ) -> PngResult<HandleResult> {
        if let Some(r) = self.text_cache_budget(length)? {
            return Ok(r);
        }
        let Some(mut buffer) = self.read_chunk_buffer(length + 1) else {
            self.crc_finish(length)?;
            self.chunk_benign_error("out of memory")?;
            return Ok(HandleResult::Error);
        };
        self.crc_read(&mut buffer[..length as usize]);
        if self.crc_finish(0)? {
            return Ok(HandleResult::Error);
        }
        buffer[length as usize] = 0;
        let key_end = buffer
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(length as usize);
        let text_start = if key_end == length as usize {
            key_end
        } else {
            key_end + 1
        };
        let key = buffer[..key_end].to_vec();
        let text = buffer[text_start.min(length as usize)..length as usize].to_vec();
        self.set_text_2(info, &key, &text, crate::structs::TextCompression::None);
        Ok(HandleResult::Ok)
    }

    /// Port of `png_handle_zTXt` (pngrutil.c#L1423-L1493).
    #[doc(alias = "png_handle_zTXt")]
    pub(crate) fn handle_ztxt(
        &mut self,
        info: &mut PngInfo,
        length: u32,
    ) -> PngResult<HandleResult> {
        if let Some(r) = self.text_cache_budget(length)? {
            return Ok(r);
        }
        let Some(mut buffer) = self.read_chunk_buffer(length) else {
            self.crc_finish(length)?;
            self.chunk_benign_error("out of memory")?;
            return Ok(HandleResult::Error);
        };
        self.crc_read(&mut buffer);
        if self.crc_finish(0)? {
            return Ok(HandleResult::Error);
        }
        let mut keyword_length = 0usize;
        while (keyword_length as u32) < length && buffer[keyword_length] != 0 {
            keyword_length += 1;
        }
        let errmsg: Option<String> = 'err: {
            if !(1..=79).contains(&keyword_length) {
                break 'err Some("bad keyword".into());
            }
            if keyword_length as u32 + 3 > length {
                break 'err Some("truncated".into());
            }
            if buffer[keyword_length + 1] != PNG_COMPRESSION_TYPE_BASE {
                break 'err Some("unknown compression type".into());
            }
            match self.decompress_chunk(&buffer, length, keyword_length + 2, true)? {
                Ok(text) => {
                    let key = buffer[..keyword_length].to_vec();
                    self.set_text_2(info, &key, &text, crate::structs::TextCompression::Ztxt);
                    return Ok(HandleResult::Ok);
                }
                Err(_) => break 'err Some(self.zstream_msg_text_opt()),
            }
        };
        self.chunk_benign_error(&errmsg.unwrap_or_default())?;
        Ok(HandleResult::Error)
    }

    /// Port of `png_handle_iTXt` (pngrutil.c#L1495-L1585).
    #[doc(alias = "png_handle_iTXt")]
    pub(crate) fn handle_itxt(
        &mut self,
        info: &mut PngInfo,
        length: u32,
    ) -> PngResult<HandleResult> {
        if let Some(r) = self.text_cache_budget(length)? {
            return Ok(r);
        }
        let Some(mut buffer) = self.read_chunk_buffer(length + 1) else {
            self.crc_finish(length)?;
            self.chunk_benign_error("out of memory")?;
            return Ok(HandleResult::Error);
        };
        self.crc_read(&mut buffer[..length as usize]);
        if self.crc_finish(0)? {
            return Ok(HandleResult::Error);
        }
        let mut prefix_length = 0usize;
        while (prefix_length as u32) < length && buffer[prefix_length] != 0 {
            prefix_length += 1;
        }
        let len = length as usize;
        let errmsg: Option<String> = 'err: {
            if !(1..=79).contains(&prefix_length) {
                break 'err Some("bad keyword".into());
            }
            if prefix_length + 5 > len {
                break 'err Some("truncated".into());
            }
            let c1 = buffer[prefix_length + 1];
            if !(c1 == 0 || (c1 == 1 && buffer[prefix_length + 2] == PNG_COMPRESSION_TYPE_BASE)) {
                break 'err Some("bad compression info".into());
            }
            let compressed = c1 != 0;
            let key = buffer[..prefix_length].to_vec();
            prefix_length += 3;
            let language_offset = prefix_length;
            while prefix_length < len && buffer[prefix_length] != 0 {
                prefix_length += 1;
            }
            let language = buffer[language_offset..prefix_length].to_vec();
            prefix_length += 1;
            let translated_keyword_offset = prefix_length;
            while prefix_length < len && buffer[prefix_length] != 0 {
                prefix_length += 1;
            }
            let translated = buffer[translated_keyword_offset..prefix_length].to_vec();
            prefix_length += 1;
            let text: Vec<u8>;
            if !compressed && prefix_length <= len {
                text = buffer[prefix_length..len].to_vec();
            } else if compressed && prefix_length < len {
                match self.decompress_chunk(&buffer, length, prefix_length, true)? {
                    Ok(t) => text = t,
                    Err(_) => break 'err Some(self.zstream_msg_text_opt()),
                }
            } else {
                break 'err Some("truncated".into());
            }
            self.set_itxt(info, &key, &language, &translated, &text, compressed);
            return Ok(HandleResult::Ok);
        };
        self.chunk_benign_error(&errmsg.unwrap_or_default())?;
        Ok(HandleResult::Error)
    }

    /// The zlib message for a failed text decompression, or the generic text.
    fn zstream_msg_text_opt(&self) -> String {
        self.zstream_msg.unwrap_or("unknown zlib error").to_owned()
    }

    /// Port of `png_decompress_chunk` (pngrutil.c#L686-L786). `input` is the whole chunk data, of
    /// `chunklength` bytes; the compressed stream starts after `prefix_size` bytes. Returns the
    /// decompressed bytes, or the zlib return code when the stream does not end within the limit.
    /// The output is limited by `png_chunk_max` less the prefix (and the terminator), and an
    /// error is returned if the stream ends with input left over (`extra compressed data`).
    pub(crate) fn decompress_chunk(
        &mut self,
        input: &[u8],
        chunklength: u32,
        prefix_size: usize,
        terminate: bool,
    ) -> PngResult<Result<Vec<u8>, ReturnCode>> {
        let term = usize::from(terminate);
        if self.user_chunk_malloc_max < prefix_size + term {
            self.zstream_error(ReturnCode::MemError);
            return Ok(Err(ReturnCode::MemError));
        }
        let limit = self.user_chunk_malloc_max - prefix_size - term;
        let ret = self.inflate_claim(self.chunk_name);
        if ret != ReturnCode::Ok {
            return Ok(Err(ret));
        }
        let lz = &input[prefix_size..chunklength as usize];
        let mut out: Vec<u8> = Vec::new();
        let mut pos = 0usize;
        let mut scratch = [0u8; PNG_INFLATE_BUF_SIZE];
        let mut ret;
        loop {
            // png_inflate with output == NULL: decode into a scratch buffer, and count the output
            // against the limit (`*output_size_ptr`).
            let want = core::cmp::min(scratch.len(), limit - out.len());
            let flush = if want > 0 {
                Flush::NoFlush
            } else {
                Flush::Finish
            };
            let (r, consumed, produced) =
                self.zlib_inflate(&lz[pos..], &mut scratch[..want], flush);
            out.extend_from_slice(&scratch[..produced]);
            pos += consumed;
            ret = r;
            if ret != ReturnCode::Ok || (consumed == 0 && produced == 0) {
                break;
            }
        }
        self.zowner = 0;
        if ret != ReturnCode::StreamEnd {
            // The C code returns Z_OK (output limit reached) as `PNG_UNEXPECTED_ZLIB_RETURN`.
            return Ok(Err(if ret == ReturnCode::Ok {
                ReturnCode::BufError
            } else {
                ret
            }));
        }
        if pos != lz.len() {
            self.chunk_benign_error("extra compressed data")?;
        }
        Ok(Ok(out))
    }
}

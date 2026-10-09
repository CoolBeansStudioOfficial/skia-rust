// Copyright (C) 1998-2025 Glenn Randers-Pehrson and the libpng contributors.
// Use of this source code is governed by the libpng licence (libpng-2.0) in the LICENSE file.
// Port of: pngset.c (libpng 1.6.56, skia.googlesource.com/third_party/libpng@d5515b5b), the
// `png_set_*` storage functions the read path calls from its chunk handlers.
//
// Only the errors that can change control flow are ported. libpng's warnings (`png_warning`)
// never change the result, so the ones these setters give are dropped. Each setter is noted
// with the C function it ports.

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
// The `png_set_*` storage functions keep libpng's signatures, which take the reader even where a
// setter stores only into the info struct.
#![allow(clippy::unused_self)]
use crate::error::PngResult;
use crate::rutil::PNG_MAX_PALETTE_LENGTH;
use crate::structs::{PngColor, PngColor8, PngColor16, PngInfo, PngStruct, info};

/// Port of `PNG_TEXT_COMPRESSION_*` and `PNG_ITXT_COMPRESSION_*` (png.h): how a text chunk was stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextCompression {
    /// `PNG_TEXT_COMPRESSION_NONE`: `tEXt`.
    None,
    /// `PNG_TEXT_COMPRESSION_zTXt`.
    Ztxt,
}

/// Port of `png_set_text_2` (pngset.c#L633): the text chunks are kept only for the info struct,
/// which the codec never reads. Stored so the struct matches libpng's contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextChunk {
    pub compression: TextCompression,
    pub key: Vec<u8>,
    pub text: Vec<u8>,
    /// Port of the `PNG_TEXT_COMPRESSION_*_WR` marks that `png_write_info` sets: the chunk has
    /// been written, so `png_write_end` does not write it again.
    pub(crate) written: bool,
}

impl PngStruct {
    /// Port of `png_set_IHDR` (pngset.c#L288-L316). The header is checked first (`png_check_IHDR`).
    #[doc(alias = "png_set_IHDR")]
    #[allow(clippy::too_many_arguments)] // mirrors png_set_IHDR's parameter list
    pub fn set_ihdr(
        &mut self,
        info: &mut PngInfo,
        width: u32,
        height: u32,
        bit_depth: u8,
        color_type: u8,
        interlace_type: u8,
        compression_type: u8,
        filter_type: u8,
    ) -> PngResult<()> {
        self.check_ihdr(
            width,
            height,
            bit_depth,
            color_type,
            interlace_type,
            compression_type,
            filter_type,
        )?;
        info.width = width;
        info.height = height;
        info.bit_depth = bit_depth;
        info.color_type = color_type;
        info.interlace_type = interlace_type;
        info.compression_type = compression_type;
        info.filter_type = filter_type;
        info.channels = match color_type {
            2 => 3,
            4 => 2,
            6 => 4,
            _ => 1,
        };
        info.pixel_depth = bit_depth * info.channels;
        info.rowbytes = crate::structs::rowbytes(info.pixel_depth, width);
        Ok(())
    }

    /// Port of `png_set_PLTE` (pngset.c#L508-L549). A zero-length palette is an error, as in C.
    #[doc(alias = "png_set_PLTE")]
    pub(crate) fn set_plte(
        &mut self,
        info: &mut PngInfo,
        palette: &[PngColor],
        num: u32,
    ) -> PngResult<()> {
        if num == 0 {
            return Err(self.error("Invalid palette"));
        }
        info.palette = palette[..num as usize].to_vec();
        info.valid |= info::PLTE;
        self.num_palette = num;
        self.palette = palette[..num as usize].to_vec();
        Ok(())
    }

    /// Port of `png_set_gAMA_fixed` (pngset.c#L244-L252).
    #[doc(alias = "png_set_gAMA_fixed")]
    pub(crate) fn set_gama_fixed(&mut self, info: &mut PngInfo, file_gamma: i32) {
        info.gamma = file_gamma as u32;
        info.valid |= info::GAMA;
    }

    /// Port of `png_set_sBIT` (pngset.c#L552-L560).
    #[doc(alias = "png_set_sBIT")]
    pub fn set_sbit(&mut self, info: &mut PngInfo, sb: PngColor8) {
        info.sig_bit = sb;
        info.valid |= info::SBIT;
    }

    /// Port of `png_set_cHRM_fixed` (pngset.c#L12-L29): the eight fixed-point values, in the order
    /// white x, white y, red x, red y, green x, green y, blue x, blue y.
    #[doc(alias = "png_set_cHRM_fixed")]
    pub(crate) fn set_chrm_fixed(&mut self, info: &mut PngInfo, vals: [i32; 8]) {
        for (dst, v) in info.int_chrm.iter_mut().zip(vals.iter()) {
            *dst = *v as u32;
        }
        info.valid |= info::CHRM;
    }

    /// Port of `png_set_sRGB` (pngset.c#L562-L569).
    #[doc(alias = "png_set_sRGB")]
    pub fn set_srgb(&mut self, info: &mut PngInfo, intent: u8) {
        info.srgb_intent = intent;
        info.valid |= info::SRGB;
    }

    /// Port of `png_set_tRNS` (pngset.c#L782-L835). `trans_alpha` holds `num_trans` alpha values
    /// for a palette image (the palette-indexed case), and `trans_color` is the colour key.
    #[doc(alias = "png_set_tRNS")]
    pub(crate) fn set_trns(
        &mut self,
        info: &mut PngInfo,
        trans_alpha: &mut [u8],
        num_trans: u16,
        trans_color: PngColor16,
    ) {
        let mut num_trans = num_trans;
        if num_trans > 0 && u32::from(num_trans) <= PNG_MAX_PALETTE_LENGTH {
            let mut padded = vec![0xffu8; PNG_MAX_PALETTE_LENGTH as usize];
            padded[..num_trans as usize].copy_from_slice(&trans_alpha[..num_trans as usize]);
            info.trans_alpha.clone_from(&padded);
            self.trans_alpha = padded;
        } else {
            info.trans_alpha = Vec::new();
            self.trans_alpha = Vec::new();
        }
        info.trans_color = trans_color;
        self.trans_color = trans_color;
        if num_trans == 0 {
            num_trans = 1;
        }
        info.num_trans = num_trans;
        self.num_trans = num_trans;
        info.valid |= info::TRNS;
    }

    /// Port of `png_set_bKGD` (pngset.c#L2-L10).
    #[doc(alias = "png_set_bKGD")]
    pub(crate) fn set_bkgd(&mut self, info: &mut PngInfo, background: PngColor16) {
        self.background = background;
        info.valid |= info::BKGD;
    }

    /// Port of `png_set_cICP` (pngset.c#L92-L109).
    #[doc(alias = "png_set_cICP")]
    pub(crate) fn set_cicp(&mut self, info: &mut PngInfo, cicp: [u8; 4]) {
        info.cicp = cicp;
        info.valid |= info::CICP;
    }

    /// Port of `png_set_cLLI_fixed` (pngset.c#L111-L126).
    #[doc(alias = "png_set_cLLI_fixed")]
    pub(crate) fn set_clli_fixed(&mut self, info: &mut PngInfo, max: u32, average: u32) {
        info.clli = [max, average];
        info.valid |= info::CLLI;
    }

    /// Port of `png_set_mDCV_fixed` (pngset.c#L147-L193).
    #[doc(alias = "png_set_mDCV_fixed")]
    pub(crate) fn set_mdcv_fixed(
        &mut self,
        info: &mut PngInfo,
        chromaticities: [u32; 8],
        max_lum: u32,
        min_lum: u32,
    ) {
        let _ = (chromaticities, max_lum, min_lum);
        info.valid |= info::MDCV;
    }

    /// Port of `png_set_eXIf_1` (pngset.c#L221-L238).
    #[doc(alias = "png_set_eXIf_1")]
    pub(crate) fn set_exif_1(&mut self, info: &mut PngInfo, data: Vec<u8>) {
        info.exif = data;
        info.valid |= info::EXIF;
    }

    /// Port of `png_set_hIST` (pngset.c#L260-L285).
    #[doc(alias = "png_set_hIST")]
    pub(crate) fn set_hist(&mut self, info: &mut PngInfo, hist: Vec<u16>) {
        self.hist = hist;
        info.valid |= info::HIST;
    }

    /// Port of `png_set_pHYs` (pngset.c#L496-L506).
    #[doc(alias = "png_set_pHYs")]
    pub(crate) fn set_phys(&mut self, info: &mut PngInfo, res_x: u32, res_y: u32, unit: u8) {
        self.phys = (res_x, res_y, unit);
        info.valid |= info::PHYS;
    }

    /// Port of `png_set_oFFs` (pngset.c#L318-L328).
    #[doc(alias = "png_set_oFFs")]
    pub(crate) fn set_offs(&mut self, info: &mut PngInfo, x: i32, y: i32, unit: u8) {
        self.offs = (x, y, unit);
        info.valid |= info::OFFS;
    }

    /// Port of `png_set_tIME` (pngset.c#L763-L780). The time is only stored; the codec does not
    /// read it.
    #[doc(alias = "png_set_tIME")]
    #[allow(clippy::too_many_arguments)] // mirrors the png_time fields
    pub(crate) fn set_time(
        &mut self,
        info: &mut PngInfo,
        year: u16,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
    ) {
        self.time = (year, month, day, hour, minute, second);
        info.valid |= info::TIME;
    }

    /// Port of `png_set_text_2` (pngset.c#L633-L760). The keyword check's warnings are dropped.
    #[doc(alias = "png_set_text_2")]
    pub(crate) fn set_text_2(
        &mut self,
        info: &mut PngInfo,
        key: &[u8],
        text: &[u8],
        compression: TextCompression,
    ) {
        self.texts.push(TextChunk {
            compression,
            key: key.to_vec(),
            text: text.to_vec(),
            written: false,
        });
        let _ = info;
    }

    /// Port of `png_set_iTXt`'s storage (`png_set_text_2` with an `iTXt` entry). Only the
    /// keyword, language and text are kept.
    #[doc(alias = "png_set_text_2")]
    pub(crate) fn set_itxt(
        &mut self,
        info: &mut PngInfo,
        key: &[u8],
        language: &[u8],
        translated: &[u8],
        text: &[u8],
        compressed: bool,
    ) {
        let _ = (language, translated);
        self.set_text_2(
            info,
            key,
            text,
            if compressed {
                TextCompression::Ztxt
            } else {
                TextCompression::None
            },
        );
    }

    /// Port of `png_set_text` (pngset.c#L1450-L1462) for the `tEXt` and `zTXt` types: stores one
    /// text chunk, to be written by `png_write_info` or `png_write_end`.
    #[doc(alias = "png_set_text")]
    pub fn set_text(
        &mut self,
        info: &mut PngInfo,
        key: &[u8],
        text: &[u8],
        compression: TextCompression,
    ) {
        self.set_text_2(info, key, text, compression);
    }

    /// Port of `png_set_iCCP` (pngset.c#L872-L925): stores the profile and its name. The profile
    /// is checked when it is written.
    #[doc(alias = "png_set_iCCP")]
    pub fn set_iccp(
        &mut self,
        info: &mut PngInfo,
        name: &str,
        compression_type: u8,
        profile: &[u8],
    ) {
        name.clone_into(&mut info.iccp_name);
        info.iccp_compression = compression_type;
        info.iccp_profile = profile.to_vec();
        info.valid |= info::ICCP;
    }

    /// Port of `png_set_unknown_chunks` (pngset.c#L1380-L1470) for a write struct: appends the
    /// chunks. A location of zero takes the current `mode` bits, as libpng does for a write struct,
    /// and the location is then reduced to its top-most bit. A location that is still zero is an
    /// error.
    #[doc(alias = "png_set_unknown_chunks")]
    pub fn set_unknown_chunks(
        &mut self,
        info: &mut PngInfo,
        unknowns: &[crate::structs::UnknownChunk],
    ) -> PngResult<()> {
        for u in unknowns {
            let mut location = u32::from(u.location) & (0x01 | 0x02 | 0x08);
            if location == 0 {
                location = self.mode & (0x01 | 0x02 | 0x08);
            }
            if location == 0 {
                return Err(self.error("invalid location in png_set_unknown_chunks"));
            }
            // Reduce the location to the top-most set bit.
            location = 1 << location.ilog2();
            info.unknown_chunks.push(crate::structs::UnknownChunk {
                name: u.name,
                data: u.data.clone(),
                location: location as u8,
            });
        }
        Ok(())
    }

    /// Port of `png_set_read_user_chunk_fn` (pngset.c#L1117-L1125): the application's reader for
    /// unknown chunks, which `png_handle_unknown` calls before keeping a chunk.
    #[doc(alias = "png_set_read_user_chunk_fn")]
    pub fn set_read_user_chunk_fn(&mut self, reader: Option<Box<dyn crate::png::UserChunkReader>>) {
        self.user_chunk = reader;
    }

    /// Port of `png_set_keep_unknown_chunks` (pngset.c#L1004-L1112). Skia passes `ALWAYS` with an
    /// empty list, which sets the default keep rule.
    #[doc(alias = "png_set_keep_unknown_chunks")]
    pub fn set_keep_unknown_chunks(&mut self, keep: u8, chunk_list: &[&[u8; 4]]) {
        if chunk_list.is_empty() {
            self.unknown_default = keep;
            return;
        }
        for name in chunk_list {
            self.chunk_list.push((name.to_vec(), keep));
        }
    }
}

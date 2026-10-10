// Copyright (C) 1991-2025, Thomas G. Lane, Guido Vollbeding and the libjpeg-turbo Project.
// Copyright (C) 2025 The skia-rust Authors.
// Use of this source code is governed by the IJG, BSD-3-Clause and zlib licences in the LICENSE file.
//
//! The 8-bit compressor of libjpeg-turbo 3.1.0 (`libjpeg_turbo@e14cbfaa`), the subset that
//! Skia's JPEG encoder (`SkJpegEncoderImpl.cpp`) calls.
//!
//! Skia's sequence is `jpeg_set_defaults`, the sampling factors (`SkJpegEncoderImpl.cpp#L160-L195`),
//! `optimize_coding = TRUE`, `jpeg_set_quality`, `jpeg_start_compress(TRUE)`,
//! `jpeg_write_marker` for the metadata segments, `jpeg_write_scanlines` and
//! `jpeg_finish_compress`. That path is ported here, from `jcapimin.c`, `jcapistd.c`,
//! `jcparam.c`, `jcinit.c`, `jcmaster.c`, `jcmarker.c`, `jccolor.c`, `jcsample.c`,
//! `jcprepct.c`, `jccoefct.c`, `jcdctmgr.c`, `jfdctint.c`, `jchuff.c` and `jstdhuff.c`.
//!
//! The bytes are those of the C library built without SIMD (the goldens' build). The forward DCT is
//! `jpeg_fdct_islow` with the reciprocal quantizer, the Huffman tables are the two-pass optimal
//! ones, and the output goes to a `Vec<u8>`: libjpeg's destination manager only decides how the
//! bytes are chunked, not what they are.
//!
//! Restructured, with the same values: libjpeg streams the image through a row buffer, a
//! downsampling pass and an iMCU-row coefficient buffer. Here the colour-converted planes, the
//! downsampled planes and the whole-image coefficient array are whole-image buffers. The edge
//! rules (replicated bottom and right padding, DC copies into dummy blocks) are the C rules, applied
//! to the same indices, so the output bytes do not depend on the buffering.
//!
//! Not ported (each returns [`Error::NotImplemented`] or [`Error::ConversionNotImplemented`]
//! rather than encoding differently): the single-pass path (`optimize_coding = FALSE`), restart
//! markers, progressive and arithmetic coding, lossless and 12/16-bit precision, `raw_data_in`,
//! scan scripts, CMYK/YCCK, RGB565 and colour quantization, and the IFAST and FLOAT DCTs. Skia
//! sets none of them.
//!
//! The crate is `unsafe`-free.

// Clippy (pedantic) allows, for this module. The code keeps the C names and arithmetic so that it can
// be checked line by line against libjpeg: integer width casts (JLONG is 32 bits, JCOEF is 16),
// the C loop shapes, and the C names (`similar_names`, `struct_field_names`).
#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::doc_markdown,
    clippy::manual_let_else,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::similar_names,
    clippy::struct_field_names,
    clippy::too_many_lines,
    clippy::unreadable_literal
)]

mod coef;
mod color;
mod fdct;
mod huff;
mod marker;
mod param;
mod sample;

use crate::error::{Error, Result};
use crate::tables::{
    ColorSpace, CompInfo, DCTSIZE, DCTSIZE2, JHuffTbl, JQuantTbl, MAX_COMPONENTS, NUM_HUFF_TBLS,
    NUM_QUANT_TBLS,
};

/// `JPEG_MAX_DIMENSION`, as a `u32`.
const MAX_DIMENSION: u32 = crate::tables::JPEG_MAX_DIMENSION;

/// `J_DCT_METHOD` as the compressor uses it. Only the integer slow DCT is ported (`JDCT_DEFAULT`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompressDctMethod {
    /// `JDCT_ISLOW`: the accurate integer forward DCT (`jfdctint.c`), libjpeg's default.
    #[default]
    IntegerSlow,
}

/// `global_state` of `jpeg_compress_struct` (`CSTATE_START`, `CSTATE_SCANNING`, and the state
/// after `jpeg_finish_compress`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GlobalState {
    Start,
    Scanning,
    Finished,
}

/// `jpeg_compress_struct`: the compressor, with the fields Skia's encoder sets or reads.
///
/// Usage follows libjpeg: [`Compress::set_image`], [`Compress::set_defaults`], any
/// [`Compress::set_quality`] / [`Compress::set_component_sampling`] / [`Compress::set_optimize_coding`],
/// [`Compress::start_compress`], then [`Compress::write_marker`] and
/// [`Compress::write_scanlines`] in any order that libjpeg allows, [`Compress::finish_compress`],
/// and [`Compress::take_output`].
// The flags are the jpeg_compress_struct's own `boolean` fields, as libjpeg names them.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct Compress {
    /// `image_width`.
    image_width: u32,
    /// `image_height`.
    image_height: u32,
    /// `input_components`.
    input_components: i32,
    /// `in_color_space`.
    in_color_space: ColorSpace,
    /// `jpeg_color_space`: the colour space written to the file.
    jpeg_color_space: ColorSpace,
    /// `num_components`.
    num_components: usize,
    /// `comp_info[MAX_COMPONENTS]`.
    comp_info: [CompInfo; MAX_COMPONENTS],
    /// `quant_tbl_ptrs[NUM_QUANT_TBLS]`.
    quant_tbl_ptrs: [Option<JQuantTbl>; NUM_QUANT_TBLS],
    /// `dc_huff_tbl_ptrs[NUM_HUFF_TBLS]`.
    dc_huff_tbl_ptrs: [Option<JHuffTbl>; NUM_HUFF_TBLS],
    /// `ac_huff_tbl_ptrs[NUM_HUFF_TBLS]`.
    ac_huff_tbl_ptrs: [Option<JHuffTbl>; NUM_HUFF_TBLS],
    /// `data_precision`.
    data_precision: i32,
    /// `optimize_coding`.
    optimize_coding: bool,
    /// `do_fancy_downsampling`.
    do_fancy_downsampling: bool,
    /// `smoothing_factor`.
    smoothing_factor: i32,
    /// `dct_method`.
    dct_method: CompressDctMethod,
    /// `restart_interval`. Only zero is ported.
    restart_interval: u32,
    /// `write_JFIF_header`.
    write_jfif_header: bool,
    /// `write_Adobe_marker`.
    write_adobe_marker: bool,
    /// `JFIF_major_version`.
    jfif_major_version: u8,
    /// `JFIF_minor_version`.
    jfif_minor_version: u8,
    /// `density_unit`.
    density_unit: u8,
    /// `X_density`.
    x_density: u16,
    /// `Y_density`.
    y_density: u16,
    /// `global_state`.
    global_state: GlobalState,
    /// `next_scanline`: the rows written so far.
    next_scanline: u32,
    /// `max_h_samp_factor`, set by `initial_setup`.
    max_h_samp_factor: i32,
    /// `max_v_samp_factor`, set by `initial_setup`.
    max_v_samp_factor: i32,
    /// `total_iMCU_rows`.
    total_imcu_rows: u32,
    /// `MCUs_per_row` of the one scan.
    mcus_per_row: u32,
    /// The colour-converted input, one full-size plane per component (`image_width` columns, the
    /// real rows only). Filled by `write_scanlines` (`jccolor.c` through `jcprepct.c`).
    planes: Vec<Vec<u8>>,
    /// `rgb_ycc_tab`: the colour product tables, built on the first converted row.
    rgb_ycc_tab: Vec<i32>,
    /// Everything written so far: SOI and the start-of-file markers, then the metadata segments,
    /// then the frame, the scan and the entropy-coded data at `finish_compress`.
    output: Vec<u8>,
}

impl Default for Compress {
    fn default() -> Self {
        Self::new()
    }
}

impl Compress {
    /// `jpeg_CreateCompress`: an empty compressor in the start state.
    pub fn new() -> Self {
        Self {
            image_width: 0,
            image_height: 0,
            input_components: 0,
            in_color_space: ColorSpace::Unknown,
            jpeg_color_space: ColorSpace::Unknown,
            num_components: 0,
            comp_info: [CompInfo::default(); MAX_COMPONENTS],
            quant_tbl_ptrs: [None; NUM_QUANT_TBLS],
            dc_huff_tbl_ptrs: [None; NUM_HUFF_TBLS],
            ac_huff_tbl_ptrs: [None; NUM_HUFF_TBLS],
            data_precision: 8,
            optimize_coding: false,
            do_fancy_downsampling: false,
            smoothing_factor: 0,
            dct_method: CompressDctMethod::IntegerSlow,
            restart_interval: 0,
            write_jfif_header: false,
            write_adobe_marker: false,
            jfif_major_version: 1,
            jfif_minor_version: 1,
            density_unit: 0,
            x_density: 1,
            y_density: 1,
            global_state: GlobalState::Start,
            next_scanline: 0,
            max_h_samp_factor: 1,
            max_v_samp_factor: 1,
            total_imcu_rows: 0,
            mcus_per_row: 0,
            planes: Vec::new(),
            rgb_ycc_tab: Vec::new(),
            output: Vec::new(),
        }
    }

    /// Sets `image_width`, `image_height`, `in_color_space` and `input_components`, the fields a
    /// caller sets before `jpeg_set_defaults`.
    pub fn set_image(
        &mut self,
        width: u32,
        height: u32,
        in_color_space: ColorSpace,
        input_components: i32,
    ) {
        self.image_width = width;
        self.image_height = height;
        self.in_color_space = in_color_space;
        self.input_components = input_components;
    }

    /// `jpeg_set_defaults`: the colour space from `in_color_space`, the standard Huffman tables,
    /// quality 75, 4:2:0-style default sampling, ISLOW DCT, fancy downsampling, no restarts.
    pub fn set_defaults(&mut self) -> Result<()> {
        self.check_state(GlobalState::Start)?;

        self.set_quality_scaled(75, true)?;
        self.std_huff_tables();
        self.optimize_coding = false;
        self.do_fancy_downsampling = true;
        self.smoothing_factor = 0;
        self.dct_method = CompressDctMethod::IntegerSlow;
        self.restart_interval = 0;
        self.jfif_major_version = 1;
        self.jfif_minor_version = 1;
        self.density_unit = 0;
        self.x_density = 1;
        self.y_density = 1;
        self.default_colorspace()
    }

    /// `jpeg_set_quality`: the quantization tables for a quality in 1..=100.
    pub fn set_quality(&mut self, quality: i32, force_baseline: bool) -> Result<()> {
        self.check_state(GlobalState::Start)?;
        self.set_quality_scaled(quality, force_baseline)
    }

    /// Sets `comp_info[index].h_samp_factor` and `v_samp_factor`, as Skia's encoder does for its
    /// 4:2:0, 4:2:2 and 4:4:4 choices.
    pub fn set_component_sampling(&mut self, index: usize, h: i32, v: i32) -> Result<()> {
        self.check_state(GlobalState::Start)?;
        if index >= self.num_components {
            return Err(Error::Internal("component index out of range"));
        }
        self.comp_info[index].h_samp_factor = h;
        self.comp_info[index].v_samp_factor = v;
        Ok(())
    }

    /// `cinfo->optimize_coding`. Only `true` is ported (Skia always sets it).
    pub fn set_optimize_coding(&mut self, optimize: bool) -> Result<()> {
        self.check_state(GlobalState::Start)?;
        self.optimize_coding = optimize;
        Ok(())
    }

    /// `cinfo->smoothing_factor` (0..=100). Non-zero values select the smoothing downsamplers.
    pub fn set_smoothing_factor(&mut self, factor: i32) -> Result<()> {
        self.check_state(GlobalState::Start)?;
        self.smoothing_factor = factor;
        Ok(())
    }

    /// `jpeg_start_compress`: `jinit_compress_master`, the start-of-file marker and the first
    /// pass setup. After it, the metadata segments can be written with [`Compress::write_marker`].
    pub fn start_compress(&mut self, write_all_tables: bool) -> Result<()> {
        self.check_state(GlobalState::Start)?;
        if write_all_tables {
            self.suppress_tables(false);
        }
        self.jinit_compress_master()?;
        self.planes = vec![Vec::new(); self.num_components];
        self.next_scanline = 0;
        self.global_state = GlobalState::Scanning;
        Ok(())
    }

    /// `jpeg_write_marker`: an APPn or COM segment with `data` as its body.
    pub fn write_marker(&mut self, marker: u8, data: &[u8]) -> Result<()> {
        if self.next_scanline != 0 || self.global_state != GlobalState::Scanning {
            return Err(Error::BadState(self.global_state as i32));
        }
        self.write_marker_header(marker, data.len() as u32)?;
        self.output.extend_from_slice(data);
        Ok(())
    }

    /// `jpeg_write_scanlines`: colour-converts and buffers `rows` (each `image_width *
    /// input_components` bytes). Returns the number of rows taken.
    pub fn write_scanlines(&mut self, rows: &[&[u8]]) -> Result<usize> {
        if self.global_state != GlobalState::Scanning {
            return Err(Error::BadState(self.global_state as i32));
        }
        if self.next_scanline >= self.image_height {
            return Ok(0);
        }
        let rows_left = (self.image_height - self.next_scanline) as usize;
        let n = rows.len().min(rows_left);
        for row in &rows[..n] {
            self.pre_process_row(row)?;
        }
        self.next_scanline += n as u32;
        Ok(n)
    }

    /// `jpeg_finish_compress`: the coefficient passes (gather the statistics, then write), the
    /// end-of-image marker. Requires every row to have been written.
    pub fn finish_compress(&mut self) -> Result<()> {
        if self.global_state != GlobalState::Scanning {
            return Err(Error::BadState(self.global_state as i32));
        }
        if self.next_scanline < self.image_height {
            return Err(Error::TooLittleData);
        }
        let planes = self.downsample_all();
        let blocks = self.forward_dct_all(&planes)?;
        // Pass 1: gather the symbol statistics and generate the optimal tables.
        let counts = self.gather_statistics(&blocks)?;
        self.finish_pass_gather(&counts)?;
        // Pass 2: write the frame, the scan header with its tables, and the entropy-coded data.
        self.write_frame_header()?;
        self.write_scan_header()?;
        let mut out = std::mem::take(&mut self.output);
        self.encode_scan(&blocks, &mut out)?;
        out.extend_from_slice(&[0xFF, marker::M_EOI]);
        self.output = out;
        self.global_state = GlobalState::Finished;
        Ok(())
    }

    /// Returns the bytes written so far and clears the buffer. After `finish_compress`, these are
    /// the whole JPEG file.
    pub fn take_output(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.output)
    }

    fn check_state(&self, expected: GlobalState) -> Result<()> {
        if self.global_state == expected {
            Ok(())
        } else {
            Err(Error::BadState(self.global_state as i32))
        }
    }

    /// `jpeg_suppress_tables`: marks every table as not yet sent.
    fn suppress_tables(&mut self, suppress: bool) {
        for q in self.quant_tbl_ptrs.iter_mut().flatten() {
            q.sent_table = suppress;
        }
        for h in self
            .dc_huff_tbl_ptrs
            .iter_mut()
            .chain(self.ac_huff_tbl_ptrs.iter_mut())
            .flatten()
        {
            h.sent_table = suppress;
        }
    }

    /// `jinit_compress_master` for the single-scan, sequential, 8-bit case: `initial_setup`, the
    /// colour converter, the downsampler and the coefficient controller are set up, then the
    /// file header is written.
    fn jinit_compress_master(&mut self) -> Result<()> {
        if !self.optimize_coding {
            return Err(Error::NotImplemented);
        }
        if self.restart_interval != 0 {
            return Err(Error::NotImplemented);
        }
        self.initial_setup()?;
        self.color_converter_check()?;
        self.downsampler_check()?;
        if self.data_precision != 8 {
            return Err(Error::BadPrecision);
        }
        self.write_file_header();
        Ok(())
    }

    /// `initial_setup` (`jcmaster.c`): the sampling maxima and the component geometry.
    fn initial_setup(&mut self) -> Result<()> {
        let data_unit = DCTSIZE as i64;
        let width = i64::from(self.image_width);
        let height = i64::from(self.image_height);
        if height <= 0 || width <= 0 || self.num_components == 0 || self.input_components <= 0 {
            return Err(Error::EmptyImage);
        }
        if height > i64::from(MAX_DIMENSION) || width > i64::from(MAX_DIMENSION) {
            return Err(Error::ImageTooBig);
        }
        let samplesperrow = width * i64::from(self.input_components);
        if samplesperrow > i64::from(u32::MAX) {
            return Err(Error::WidthOverflow);
        }
        if self.num_components > MAX_COMPONENTS {
            return Err(Error::ComponentCount(
                self.num_components as i32,
                MAX_COMPONENTS as i32,
            ));
        }
        self.max_h_samp_factor = 1;
        self.max_v_samp_factor = 1;
        for compptr in &self.comp_info[..self.num_components] {
            if compptr.h_samp_factor <= 0
                || compptr.h_samp_factor > crate::tables::MAX_SAMP_FACTOR
                || compptr.v_samp_factor <= 0
                || compptr.v_samp_factor > crate::tables::MAX_SAMP_FACTOR
            {
                return Err(Error::BadSampling);
            }
            self.max_h_samp_factor = self.max_h_samp_factor.max(compptr.h_samp_factor);
            self.max_v_samp_factor = self.max_v_samp_factor.max(compptr.v_samp_factor);
        }
        let max_h = i64::from(self.max_h_samp_factor);
        let max_v = i64::from(self.max_v_samp_factor);
        for ci in 0..self.num_components {
            let compptr = &mut self.comp_info[ci];
            compptr.component_index = ci as i32;
            compptr.dct_h_scaled_size = DCTSIZE as i32;
            compptr.dct_v_scaled_size = DCTSIZE as i32;
            let h = i64::from(compptr.h_samp_factor);
            let v = i64::from(compptr.v_samp_factor);
            compptr.width_in_blocks = jdiv_round_up(width * h, max_h * data_unit) as u32;
            compptr.height_in_blocks = jdiv_round_up(height * v, max_v * data_unit) as u32;
            compptr.downsampled_width = jdiv_round_up(width * h, max_h) as u32;
            compptr.downsampled_height = jdiv_round_up(height * v, max_v) as u32;
            compptr.component_needed = true;
        }
        self.total_imcu_rows = jdiv_round_up(height, max_v * data_unit) as u32;
        // per_scan_setup: the MCU geometry of the one scan.
        if self.num_components == 1 {
            self.mcus_per_row = self.comp_info[0].width_in_blocks;
        } else {
            let mut blocks_in_mcu = 0;
            for compptr in &self.comp_info[..self.num_components] {
                blocks_in_mcu += compptr.h_samp_factor * compptr.v_samp_factor;
            }
            if blocks_in_mcu > 10 {
                return Err(Error::BadMcuSize);
            }
            self.mcus_per_row = jdiv_round_up(width, max_h * data_unit) as u32;
        }
        Ok(())
    }

    /// `_jinit_color_converter` (`jccolor.c`): which colour spaces may be converted to which.
    fn color_converter_check(&self) -> Result<()> {
        if self.data_precision != 8 {
            return Err(Error::BadPrecision);
        }
        self.color_conversion_kind().map(|_| ())
    }

    /// `_jinit_downsampler` (`jcsample.c`): the method for each component, or an error. Returns
    /// nothing; the methods are chosen again by [`Compress::downsample_all`].
    fn downsampler_check(&self) -> Result<()> {
        let maxh = self.max_h_samp_factor;
        let maxv = self.max_v_samp_factor;
        for compptr in &self.comp_info[..self.num_components] {
            let h = compptr.h_samp_factor;
            let v = compptr.v_samp_factor;
            // fullsize, h2v1, h2v2, or an integer box reduction (`_jinit_downsampler`).
            let supported = (h * 2 == maxh || h == maxh) && (v == maxv || v * 2 == maxv)
                || (maxh % h == 0 && maxv % v == 0);
            if !supported {
                return Err(Error::FractSampleNotImplemented);
            }
        }
        Ok(())
    }
    /// `jpeg_set_quality` through `jpeg_quality_scaling` and `jpeg_set_linear_quality`.
    fn set_quality_scaled(&mut self, quality: i32, force_baseline: bool) -> Result<()> {
        let scale = Self::quality_scaling(quality);
        self.set_linear_quality(scale, force_baseline)
    }

    /// `jpeg_quality_scaling` (`jcparam.c`).
    pub(crate) fn quality_scaling(quality: i32) -> i32 {
        let mut quality = quality;
        if quality <= 0 {
            quality = 1;
        }
        if quality > 100 {
            quality = 100;
        }
        if quality < 50 {
            5000 / quality
        } else {
            200 - quality * 2
        }
    }

    /// `write_file_header` (`jcmarker.c`): SOI, then the JFIF and Adobe markers when they apply.
    fn write_file_header(&mut self) {
        self.output.extend_from_slice(&[0xFF, marker::M_SOI]);
        if self.write_jfif_header {
            self.emit_jfif_app0();
        }
        if self.write_adobe_marker {
            self.emit_adobe_app14();
        }
    }
}

/// `jdiv_round_up` (`jutils.c`): `ceil(a / b)` for positive `b`.
pub(crate) fn jdiv_round_up(a: i64, b: i64) -> i64 {
    (a + b - 1) / b
}

/// `jround_up` (`jutils.c`): `a` rounded up to a multiple of `b`.
pub(crate) fn jround_up(a: i64, b: i64) -> i64 {
    jdiv_round_up(a, b) * b
}

/// `DCTSIZE`-indexed block of 64 coefficients (`JBLOCK`), in natural order.
pub(crate) type Block = [i16; DCTSIZE2];

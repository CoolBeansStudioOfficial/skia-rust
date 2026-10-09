// Port of: libjpeg-turbo src/jdapimin.c#L1-L421 and src/jdapistd.c#L1-L702 (libjpeg_turbo@e14cbfaa, 3.1.0)
//
// Copyright (C) 1994-1998, Thomas G. Lane. Modified 2009-2017 by Guido Vollbeding.
// libjpeg-turbo Modifications: Copyright (C) 2015-2016, 2019, 2022-2023, D. R. Commander.
// Rust port Copyright (C) 2025 The skia-rust Authors. Licence: IJG (see LICENSE).
//
//! The decompressor object (`jpeg_decompress_struct`) and the public API that Skia calls.
//!
//! libjpeg keeps one large struct that all its modules share; here it is [`Decompress`], and the
//! modules are `impl` blocks over it. The API functions keep their C names in snake case:
//! `jpeg_read_header` → [`Decompress::read_header`], `jpeg_start_decompress` →
//! [`Decompress::start_decompress`], and so on.

// Clippy (pedantic) allows, for this module. Each one fires on the C arithmetic and naming this
// module mirrors, and the code is kept as the C writes it so it can be checked line by line:
// JLONG/int/JDIMENSION casts (sign, truncation and wrap), C operator precedence and identity
// terms that come out of macros (`x * 1`, `0 * n`), C loop shapes (`needless_range_loop`,
// `explicit_counter_loop`, `collapsible_if`, `match_same_arms`), the C variable names
// (`similar_names`, `struct_field_names`), libjpeg's constants written as in jdct.h
// (`approx_constant`, `unreadable_literal`), functions whose C form returns a status that
// this path never sets (`unnecessary_wraps`), and the long C routines (`too_many_lines`,
// `too_many_arguments`). Error docs point at the `Error` variants, which name the C codes.
#![allow(
    clippy::approx_constant,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::collapsible_if,
    clippy::doc_markdown,
    clippy::erasing_op,
    clippy::explicit_counter_loop,
    clippy::identity_op,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::needless_range_loop,
    clippy::precedence,
    clippy::similar_names,
    clippy::single_match_else,
    clippy::struct_field_names,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    clippy::unreadable_literal,
    clippy::unused_self
)]

use std::sync::Arc;

use crate::coef::CoefState;
use crate::coef_buf::{CompCoefs, SAVED_COEFS};
use crate::color::ColorState;
use crate::error::{Error, Result};
use crate::huff::HuffDecoder;
use crate::idct::RangeLimit;
use crate::input::InputState;
use crate::main_ctl::MainState;
use crate::marker::{ConsumeResult, MarkerReader, SavedMarker};
use crate::master::MasterState;
use crate::source::{JpegSource, SrcBuf};
use crate::tables::{
    ColorSpace, CompInfo, DCTSIZE2, JHuffTbl, JQuantTbl, NUM_ARITH_TBLS, NUM_HUFF_TBLS,
    NUM_QUANT_TBLS,
};
use crate::upsample::UpsampleState;

/// `dither_mode` (`J_DITHER_MODE`). Skia only uses `JDITHER_NONE` (RGB565 output).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DitherMode {
    /// `JDITHER_NONE`.
    None,
    /// `JDITHER_ORDERED`.
    Ordered,
    /// `JDITHER_FS` (libjpeg's default).
    #[default]
    FloydSteinberg,
}

/// `DCT_method` (`J_DCT_METHOD`). Skia keeps the default, `JDCT_ISLOW`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DctMethod {
    /// `JDCT_ISLOW`: accurate integer method (the default).
    #[default]
    IsLow,
    /// `JDCT_IFAST`: not ported.
    IFast,
    /// `JDCT_FLOAT`: not ported.
    Float,
}

/// `global_state` (`DSTATE_*` in `jpegint.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum GlobalState {
    #[default]
    Start,
    Inheader,
    Ready,
    Preload,
    Prescan,
    Scanning,
    RawOk,
    BufImage,
    BufPost,
    Stopping,
}

/// Result of [`Decompress::read_header`] (`JPEG_HEADER_*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderResult {
    /// `JPEG_HEADER_OK`: an image was found.
    Ok,
    /// `JPEG_HEADER_TABLES_ONLY`: only tables were found (only when `require_image` is false).
    TablesOnly,
    /// `JPEG_SUSPENDED`: more input is needed.
    Suspended,
}

/// The decompressor. Fields are public so that a caller (Skia's `SkJpegCodec` port) can read and
/// set the parameters that `jpeglib.h` exposes; the state machine is private.
#[allow(clippy::struct_excessive_bools)] // mirrors jpeg_decompress_struct's flag fields
pub struct Decompress {
    /// `image_width`, `image_height`: the frame size from SOF.
    pub image_width: u32,
    /// Frame height from SOF.
    pub image_height: u32,
    /// `data_precision`: always 8 for supported images.
    pub data_precision: i32,
    /// `num_components`.
    pub num_components: i32,
    /// `jpeg_color_space`: the colour space of the JPEG data.
    pub jpeg_color_space: ColorSpace,
    /// `out_color_space`: the colour space of the output.
    pub out_color_space: ColorSpace,
    /// `scale_num / scale_denom`: output scaling (1/8 .. 8/8 for Skia).
    pub scale_num: u32,
    /// Output scaling denominator.
    pub scale_denom: u32,
    /// `output_width`, `output_height`: the output size after scaling.
    pub output_width: u32,
    /// Output height.
    pub output_height: u32,
    /// `out_color_components`.
    pub out_color_components: i32,
    /// `output_components`.
    pub output_components: i32,
    /// `rec_outbuf_height`: recommended number of output rows per call (1 here).
    pub rec_outbuf_height: i32,
    /// `output_scanline`: the next output row.
    pub output_scanline: u32,
    /// `output_scan_number`.
    pub output_scan_number: i32,
    /// `input_scan_number`: number of SOS markers seen.
    pub input_scan_number: i32,
    /// `input_iMCU_row`, `output_iMCU_row`.
    pub input_imcu_row: u32,
    /// Output iMCU row counter.
    pub output_imcu_row: u32,
    /// `total_iMCU_rows`.
    pub total_imcu_rows: u32,
    /// `buffered_image`: the multi-pass (progressive) API.
    pub buffered_image: bool,
    /// `raw_data_out`: return downsampled YCbCr planes (`jpeg_read_raw_data`).
    pub raw_data_out: bool,
    /// `dct_method`.
    pub dct_method: DctMethod,
    /// `do_fancy_upsampling` (default TRUE).
    pub do_fancy_upsampling: bool,
    /// `do_block_smoothing` (default TRUE).
    pub do_block_smoothing: bool,
    /// `dither_mode` (default `JDITHER_FS`; Skia sets `None` for RGB565).
    pub dither_mode: DitherMode,
    /// `restart_interval` from DRI.
    pub restart_interval: u32,
    /// `progressive_mode` (SOF2 or SOF10).
    pub progressive_mode: bool,
    /// `arith_code` (SOF9..SOF11).
    pub arith_code: bool,
    /// `arith_dc_L` / `arith_dc_U` / `arith_ac_K` from DAC.
    pub arith_dc_l: [u8; NUM_ARITH_TBLS],
    /// `arith_dc_U`.
    pub arith_dc_u: [u8; NUM_ARITH_TBLS],
    /// `arith_ac_K`.
    pub arith_ac_k: [u8; NUM_ARITH_TBLS],
    /// `ccir601_sampling`: unsupported (always false).
    pub ccir601_sampling: bool,
    /// `saw_JFIF_marker` and its fields.
    pub saw_jfif_marker: bool,
    /// `JFIF_major_version`.
    pub jfif_major_version: u8,
    /// `JFIF_minor_version`.
    pub jfif_minor_version: u8,
    /// `density_unit`.
    pub density_unit: u8,
    /// `X_density`.
    pub x_density: u16,
    /// `Y_density`.
    pub y_density: u16,
    /// `saw_Adobe_marker` and `Adobe_transform`.
    pub saw_adobe_marker: bool,
    /// `Adobe_transform`.
    pub adobe_transform: u8,
    /// `comp_info`: the frame components.
    pub comp_info: Vec<CompInfo>,
    /// `max_h_samp_factor`, `max_v_samp_factor`.
    pub max_h_samp_factor: i32,
    /// Largest vertical sampling factor.
    pub max_v_samp_factor: i32,
    /// `min_DCT_h_scaled_size` (`_min_DCT_scaled_size`): smallest scaled DCT size.
    pub min_dct_scaled_size: i32,
    /// `comps_in_scan`.
    pub comps_in_scan: i32,
    /// `cur_comp_info[]`: indices into `comp_info` for the current scan.
    pub cur_comp_info: [Option<usize>; crate::tables::MAX_COMPS_IN_SCAN],
    /// `Ss`, `Se`, `Ah`, `Al`.
    pub ss: i32,
    /// Spectral selection end.
    pub se: i32,
    /// Successive approximation high bit.
    pub ah: i32,
    /// Successive approximation low bit.
    pub al: i32,
    /// `blocks_in_MCU`.
    pub blocks_in_mcu: i32,
    /// `MCU_membership[]`.
    pub mcu_membership: [i32; crate::tables::D_MAX_BLOCKS_IN_MCU],
    /// `MCUs_per_row`.
    pub mcus_per_row: u32,
    /// `MCU_rows_in_scan`.
    pub mcu_rows_in_scan: u32,
    /// `quant_tbl_ptrs[]`.
    pub quant_tbl_ptrs: [Option<JQuantTbl>; NUM_QUANT_TBLS],
    /// `dc_huff_tbl_ptrs[]`.
    pub dc_huff_tbl_ptrs: [Option<JHuffTbl>; NUM_HUFF_TBLS],
    /// `ac_huff_tbl_ptrs[]`.
    pub ac_huff_tbl_ptrs: [Option<JHuffTbl>; NUM_HUFF_TBLS],
    /// `marker_list`: the saved APPn/COM segments, in file order.
    pub marker_list: Vec<SavedMarker>,
    /// `global_state`.
    pub(crate) global_state: GlobalState,
    /// `err->num_warnings`: how many warnings were raised (they do not stop decoding).
    pub num_warnings: u32,
    /// `err->msg_code` of the last warning.
    pub last_warning: i32,
    /// The data source (`cinfo->src`).
    pub(crate) source: Option<Box<dyn JpegSource>>,
    /// The source's `next_input_byte` / `bytes_in_buffer`.
    pub(crate) srcbuf: SrcBuf,
    /// `unread_marker` (0 when none).
    pub(crate) unread_marker: i32,
    /// `insufficient_data` (entropy decoder flag).
    pub(crate) insufficient_data: bool,
    /// `sample_range_limit`.
    pub(crate) range_limit: Arc<RangeLimit>,
    /// Marker reader state.
    pub(crate) marker: MarkerReader,
    /// Input controller state.
    pub(crate) inputctl: InputState,
    /// Master controller state.
    pub(crate) master: MasterState,
    /// Huffman entropy decoder state.
    pub(crate) huff: HuffDecoder,
    /// `coef_bits[ci][coefi]`: the bit position each coefficient was last coded at, per
    /// component (`ci`) and, from `ci + num_components`, for the previous scan.
    pub(crate) coef_bits: Vec<[i32; DCTSIZE2]>,
    /// `coef_bits_latch`: the bit positions block smoothing uses for this output pass.
    pub(crate) coef_bits_latch: Vec<[i32; SAVED_COEFS]>,
    /// `whole_image`: the coefficients of the whole image (multi-pass decoding only).
    pub(crate) whole_image: Vec<CompCoefs>,
    /// `coef->coef_arrays != NULL`: the coefficient controller buffers the whole image.
    pub(crate) coef_buffered: bool,
    /// The output pass uses `decompress_smooth_data` (block smoothing applies).
    pub(crate) coef_smooth: bool,
    /// Coefficient buffer controller state.
    pub(crate) coef: CoefState,
    /// Main buffer controller state.
    pub(crate) main: MainState,
    /// Upsampler state.
    pub(crate) upsample: UpsampleState,
    /// Colour converter state.
    pub(crate) cconvert: ColorState,
    /// Per-component dequantization tables latched at the start of each scan.
    pub(crate) dct_tables: Vec<[i32; 64]>,
    /// Per-component inverse DCT method size (`_inverse_DCT[ci]`).
    pub(crate) idct_size: Vec<i32>,
}

impl Decompress {
    /// `jpeg_create_decompress`: a decompressor that reads from `source`.
    pub fn new(source: Box<dyn JpegSource>) -> Self {
        let mut d = Decompress {
            image_width: 0,
            image_height: 0,
            data_precision: 8,
            num_components: 0,
            jpeg_color_space: ColorSpace::Unknown,
            out_color_space: ColorSpace::Unknown,
            scale_num: 1,
            scale_denom: 1,
            output_width: 0,
            output_height: 0,
            out_color_components: 0,
            output_components: 0,
            rec_outbuf_height: 1,
            output_scanline: 0,
            output_scan_number: 0,
            input_scan_number: 0,
            input_imcu_row: 0,
            output_imcu_row: 0,
            total_imcu_rows: 0,
            buffered_image: false,
            raw_data_out: false,
            dct_method: DctMethod::IsLow,
            do_fancy_upsampling: true,
            do_block_smoothing: true,
            dither_mode: DitherMode::FloydSteinberg,
            restart_interval: 0,
            progressive_mode: false,
            arith_code: false,
            arith_dc_l: [0; NUM_ARITH_TBLS],
            arith_dc_u: [0; NUM_ARITH_TBLS],
            arith_ac_k: [0; NUM_ARITH_TBLS],
            ccir601_sampling: false,
            saw_jfif_marker: false,
            jfif_major_version: 1,
            jfif_minor_version: 1,
            density_unit: 0,
            x_density: 1,
            y_density: 1,
            saw_adobe_marker: false,
            adobe_transform: 0,
            comp_info: Vec::new(),
            max_h_samp_factor: 1,
            max_v_samp_factor: 1,
            min_dct_scaled_size: 1,
            comps_in_scan: 0,
            cur_comp_info: [None; crate::tables::MAX_COMPS_IN_SCAN],
            ss: 0,
            se: 0,
            ah: 0,
            al: 0,
            blocks_in_mcu: 0,
            mcu_membership: [0; crate::tables::D_MAX_BLOCKS_IN_MCU],
            mcus_per_row: 0,
            mcu_rows_in_scan: 0,
            quant_tbl_ptrs: [None, None, None, None],
            dc_huff_tbl_ptrs: [None, None, None, None],
            ac_huff_tbl_ptrs: [None, None, None, None],
            marker_list: Vec::new(),
            global_state: GlobalState::Start,
            num_warnings: 0,
            last_warning: 0,
            source: Some(source),
            srcbuf: SrcBuf::default(),
            unread_marker: 0,
            insufficient_data: false,
            range_limit: Arc::new(RangeLimit::new()),
            marker: MarkerReader::default(),
            inputctl: InputState::default(),
            master: MasterState::default(),
            huff: HuffDecoder::default(),
            coef_bits: Vec::new(),
            coef_bits_latch: Vec::new(),
            whole_image: Vec::new(),
            coef_buffered: false,
            coef_smooth: false,
            coef: CoefState::default(),
            main: MainState::default(),
            upsample: UpsampleState::default(),
            cconvert: ColorState::default(),
            dct_tables: Vec::new(),
            idct_size: Vec::new(),
        };
        d.init_marker_reader();
        d.inputctl = InputState::default();
        d.inputctl.inheaders = true;
        d.global_state = GlobalState::Start;
        d
    }

    /// `output_scanline`: the next output row.
    pub fn output_scanline(&self) -> u32 {
        self.output_scanline
    }

    /// `output_height`: the output height after scaling.
    pub fn output_height(&self) -> u32 {
        self.output_height
    }

    /// `dct_method`.
    pub fn dct_method(&self) -> DctMethod {
        self.dct_method
    }

    /// `jpeg_read_header`: reads markers up to the first SOS (or EOI).
    pub fn read_header(&mut self, require_image: bool) -> Result<HeaderResult> {
        if self.global_state != GlobalState::Start && self.global_state != GlobalState::Inheader {
            return Err(Error::BadState(self.global_state as i32));
        }
        let retcode = self.consume_input()?;
        match retcode {
            ConsumeResult::ReachedSos => Ok(HeaderResult::Ok),
            ConsumeResult::ReachedEoi => {
                if require_image {
                    return Err(Error::NoImage);
                }
                self.abort();
                Ok(HeaderResult::TablesOnly)
            }
            ConsumeResult::Suspended => Ok(HeaderResult::Suspended),
            _ => Ok(HeaderResult::Suspended),
        }
    }

    /// `jpeg_consume_input`: reads more of the file, returning the marker-level result.
    pub fn consume_input(&mut self) -> Result<ConsumeResult> {
        let retcode = match self.global_state {
            GlobalState::Start => {
                self.reset_input_controller();
                let mut buf = std::mem::take(&mut self.srcbuf);
                if let Some(src) = self.source.as_mut() {
                    src.init_source(&mut buf);
                }
                self.srcbuf = buf;
                self.global_state = GlobalState::Inheader;
                self.consume_input_inheader()?
            }
            GlobalState::Inheader => self.consume_input_inheader()?,
            GlobalState::Ready => ConsumeResult::ReachedSos,
            GlobalState::Preload
            | GlobalState::Prescan
            | GlobalState::Scanning
            | GlobalState::RawOk
            | GlobalState::BufImage
            | GlobalState::BufPost
            | GlobalState::Stopping => self.inputctl_consume()?,
        };
        Ok(retcode)
    }

    /// The `consume_input` of the input controller in header state (`inheaders`).
    fn consume_input_inheader(&mut self) -> Result<ConsumeResult> {
        let retcode = self.inputctl_consume()?;
        if retcode == ConsumeResult::ReachedSos {
            // default_decompress_parms: set the colour spaces from the header.
            self.default_decompress_parms()?;
            self.global_state = GlobalState::Ready;
        }
        Ok(retcode)
    }

    /// `jpeg_abort_decompress` (used after TABLES_ONLY): back to the start state.
    pub fn abort(&mut self) {
        self.global_state = GlobalState::Start;
        self.reset_marker_reader();
        self.inputctl = InputState::default();
        self.inputctl.inheaders = true;
    }

    /// `jpeg_input_complete`: whether the end of the image has been read.
    pub fn input_complete(&self) -> bool {
        self.inputctl.eoi_reached
    }

    /// `jpeg_has_multiple_scans`.
    pub fn has_multiple_scans(&self) -> bool {
        self.inputctl.has_multiple_scans
    }

    /// `jpeg_calc_output_dimensions`: sets `output_width`/`output_height` for the current
    /// `scale_num`/`scale_denom` and `out_color_space`.
    pub fn calc_output_dimensions(&mut self) -> Result<()> {
        if self.global_state != GlobalState::Ready {
            return Err(Error::BadState(self.global_state as i32));
        }
        self.calc_output_dimensions_full()
    }

    /// `jpeg_start_decompress`: prepares the output pass. Returns `Ok(false)` on suspension.
    pub fn start_decompress(&mut self) -> Result<bool> {
        if self.global_state == GlobalState::Ready {
            self.jinit_master_decompress()?;
            if self.buffered_image {
                self.global_state = GlobalState::BufImage;
                return Ok(true);
            }
            self.global_state = GlobalState::Preload;
        }
        if self.global_state == GlobalState::Preload {
            if self.inputctl.has_multiple_scans {
                // Consume the whole file before output, as the multi-scan path does.
                loop {
                    let retcode = self.consume_input()?;
                    match retcode {
                        ConsumeResult::Suspended => return Ok(false),
                        ConsumeResult::ReachedEoi => break,
                        _ => {}
                    }
                }
            }
            self.output_scan_number = self.input_scan_number;
        } else if self.global_state != GlobalState::Prescan {
            return Err(Error::BadState(self.global_state as i32));
        }
        self.output_pass_setup()
    }

    /// `output_pass_setup`.
    fn output_pass_setup(&mut self) -> Result<bool> {
        if self.global_state != GlobalState::Prescan {
            self.prepare_for_output_pass()?;
            self.output_scanline = 0;
            self.global_state = GlobalState::Prescan;
        }
        // No quantization pre-pass (quantize_colors is never set by Skia).
        self.global_state = if self.raw_data_out {
            GlobalState::RawOk
        } else {
            GlobalState::Scanning
        };
        Ok(true)
    }

    /// `jpeg_read_scanlines`: decodes up to `scanlines.len()` rows into the given buffers.
    /// Returns the number of rows produced (0 at the end of the image).
    pub fn read_scanlines(&mut self, scanlines: &mut [&mut [u8]]) -> Result<usize> {
        if self.data_precision != 8 {
            return Err(Error::BadPrecision);
        }
        if self.global_state != GlobalState::Scanning {
            return Err(Error::BadState(self.global_state as i32));
        }
        if self.output_scanline >= self.output_height {
            return Ok(0);
        }
        let rows = self.process_data_into(scanlines)?;
        self.output_scanline += rows as u32;
        Ok(rows)
    }

    /// `jpeg_read_raw_data`: decodes one iMCU row of downsampled planes. `planes[ci]` must hold
    /// at least `max_v_samp_factor * DCTSIZE` rows of `width_in_blocks * DCTSIZE` samples.
    pub fn read_raw_data(&mut self, planes: &mut [Vec<Vec<u8>>]) -> Result<usize> {
        if self.data_precision != 8 {
            return Err(Error::BadPrecision);
        }
        if self.global_state != GlobalState::RawOk {
            return Err(Error::BadState(self.global_state as i32));
        }
        if self.output_scanline >= self.output_height {
            return Ok(0);
        }
        let lines_per_imcu_row = self.max_v_samp_factor * self.min_dct_scaled_size;
        if (planes.first().map_or(0, Vec::len) as i32) < lines_per_imcu_row {
            return Err(Error::Internal("raw buffer too small"));
        }
        if !self.decompress_data_raw(planes)? {
            return Ok(0);
        }
        self.output_scanline += lines_per_imcu_row as u32;
        Ok(lines_per_imcu_row as usize)
    }

    /// `jpeg_finish_decompress`: reads the rest of the input. Returns `Ok(false)` on suspension.
    pub fn finish_decompress(&mut self) -> Result<bool> {
        if (self.global_state == GlobalState::Scanning || self.global_state == GlobalState::RawOk)
            && !self.buffered_image
        {
            if self.output_scanline < self.output_height {
                return Err(Error::TooLittleData);
            }
            self.finish_output_pass()?;
            self.global_state = GlobalState::Stopping;
        } else if self.global_state == GlobalState::BufImage {
            self.global_state = GlobalState::Stopping;
        } else if self.global_state != GlobalState::Stopping {
            return Err(Error::BadState(self.global_state as i32));
        }
        while !self.inputctl.eoi_reached {
            if self.consume_input()? == ConsumeResult::Suspended {
                return Ok(false);
            }
        }
        self.abort();
        Ok(true)
    }

    /// `jpeg_start_output`: starts an output pass in buffered-image mode, for the input up to and
    /// including scan `scan_number`.
    pub fn start_output(&mut self, scan_number: i32) -> Result<()> {
        if self.global_state != GlobalState::BufImage && self.global_state != GlobalState::Prescan {
            return Err(Error::BadState(self.global_state as i32));
        }
        self.output_scan_number = scan_number;
        self.output_pass_setup()?;
        Ok(())
    }

    /// `jpeg_finish_output`: ends the output pass. Returns `Ok(false)` on suspension, in which
    /// case the call is repeated.
    pub fn finish_output(&mut self) -> Result<bool> {
        if (self.global_state == GlobalState::Scanning || self.global_state == GlobalState::RawOk)
            && self.buffered_image
        {
            // Terminate this pass: the whole pass need not have been read.
            self.global_state = GlobalState::BufPost;
        } else if self.global_state != GlobalState::BufPost {
            return Err(Error::BadState(self.global_state as i32));
        }
        // Read markers looking for SOS or EOI.
        while self.input_scan_number <= self.output_scan_number && !self.inputctl.eoi_reached {
            if self.inputctl_consume()? == ConsumeResult::Suspended {
                return Ok(false);
            }
        }
        self.global_state = GlobalState::BufImage;
        Ok(true)
    }

    /// `jpeg_destroy_decompress`: releases the state (kept for API symmetry).
    pub fn destroy(self) {}
}

impl std::fmt::Debug for Decompress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Decompress")
            .field("image_width", &self.image_width)
            .field("image_height", &self.image_height)
            .field("num_components", &self.num_components)
            .field("jpeg_color_space", &self.jpeg_color_space)
            .field("out_color_space", &self.out_color_space)
            .field("scale_num", &self.scale_num)
            .field("scale_denom", &self.scale_denom)
            .field("output_width", &self.output_width)
            .field("output_height", &self.output_height)
            .finish_non_exhaustive()
    }
}

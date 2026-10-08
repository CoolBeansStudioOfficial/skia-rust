// Port of: libjpeg-turbo src/jdinput.c#L29-L424 and the colour-space defaults of
//          src/jdapimin.c#L129-L234 (libjpeg_turbo@e14cbfaa, 3.1.0)
//
// Copyright (C) 1991-1997, Thomas G. Lane. libjpeg-turbo Modifications: Copyright (C) 2010,
// 2016, 2019, D. R. Commander. Rust port Copyright (C) 2025 The skia-rust Authors. Licence: IJG.
//
//! The input controller: consumes markers until the first SOS, then hands control to the
//! coefficient controller for each scan, and sets the per-image and per-scan geometry.

use crate::Decompress;
use crate::error::{Error, Result};
use crate::marker::ConsumeResult;
use crate::tables::{ColorSpace, DCTSIZE, D_MAX_BLOCKS_IN_MCU, JPEG_MAX_DIMENSION, MAX_COMPONENTS, MAX_COMPS_IN_SCAN, MAX_SAMP_FACTOR};

/// `inputctl->consume_input`: which routine the input controller dispatches to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ConsumeKind {
    /// `consume_markers`: read markers (header or between scans).
    #[default]
    Markers,
    /// `coef->consume_data`: read the coefficients of the current scan.
    CoefData,
}

/// `my_input_controller` plus the public fields of `jpeg_input_controller`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct InputState {
    pub(crate) consume: ConsumeKind,
    /// `inheaders`: still before the first SOS.
    pub(crate) inheaders: bool,
    /// `eoi_reached`.
    pub(crate) eoi_reached: bool,
    /// `has_multiple_scans`.
    pub(crate) has_multiple_scans: bool,
}

impl Decompress {
    /// `reset_input_controller` (called at DSTATE_START).
    pub(crate) fn reset_input_controller(&mut self) {
        self.inputctl.consume = ConsumeKind::Markers;
        self.inputctl.has_multiple_scans = false;
        self.inputctl.eoi_reached = false;
        self.inputctl.inheaders = true;
        self.reset_marker_reader();
    }

    /// `consume_input` of the input controller (dispatches on `inputctl.consume`).
    pub(crate) fn inputctl_consume(&mut self) -> Result<ConsumeResult> {
        match self.inputctl.consume {
            ConsumeKind::Markers => self.consume_markers(),
            ConsumeKind::CoefData => self.coef_consume_data(),
        }
    }

    /// `consume_markers`.
    fn consume_markers(&mut self) -> Result<ConsumeResult> {
        if self.inputctl.eoi_reached {
            return Ok(ConsumeResult::ReachedEoi);
        }
        let val = self.read_markers()?;
        match val {
            ConsumeResult::ReachedSos => {
                if self.inputctl.inheaders {
                    // First SOS: the frame is known, so set up the image-level geometry.
                    self.initial_setup()?;
                    self.inputctl.inheaders = false;
                    // Check the number of progressive scans is correct.
                    // (multi-scan detection happens in initial_setup)
                } else {
                    if !self.inputctl.has_multiple_scans {
                        return Err(Error::BadState(-1));
                    }
                    self.start_input_pass()?;
                }
            }
            ConsumeResult::ReachedEoi => {
                self.inputctl.eoi_reached = true;
                if self.inputctl.inheaders {
                    if self.marker.saw_sof {
                        return Err(Error::Internal("SOF without SOS"));
                    }
                } else if self.output_scan_number > self.input_scan_number {
                    self.output_scan_number = self.input_scan_number;
                }
            }
            ConsumeResult::Suspended => {}
            _ => {}
        }
        Ok(val)
    }

    /// `initial_setup`: image-level geometry, from the frame header (called at the first SOS).
    fn initial_setup(&mut self) -> Result<()> {
        let data_unit = DCTSIZE as i32;
        // Check the frame size: libjpeg rejects images larger than JPEG_MAX_DIMENSION.
        if i64::from(self.image_height) > i64::from(JPEG_MAX_DIMENSION)
            || i64::from(self.image_width) > i64::from(JPEG_MAX_DIMENSION)
        {
            return Err(Error::ImageTooBig);
        }
        if self.data_precision != 8 && self.data_precision != 12 {
            return Err(Error::BadPrecision);
        }
        if self.num_components as usize > MAX_COMPONENTS {
            return Err(Error::BadSampling);
        }
        self.max_h_samp_factor = 1;
        self.max_v_samp_factor = 1;
        for ci in 0..self.num_components as usize {
            let c = self.comp_info[ci];
            if c.h_samp_factor <= 0
                || c.h_samp_factor > MAX_SAMP_FACTOR
                || c.v_samp_factor <= 0
                || c.v_samp_factor > MAX_SAMP_FACTOR
            {
                return Err(Error::BadSampling);
            }
            self.max_h_samp_factor = self.max_h_samp_factor.max(c.h_samp_factor);
            self.max_v_samp_factor = self.max_v_samp_factor.max(c.v_samp_factor);
        }
        self.min_dct_scaled_size = data_unit;
        for ci in 0..self.num_components as usize {
            self.comp_info[ci].dct_h_scaled_size = data_unit;
            self.comp_info[ci].dct_v_scaled_size = data_unit;
            let c = self.comp_info[ci];
            self.comp_info[ci].width_in_blocks = jdiv_round_up(
                i64::from(self.image_width) * i64::from(c.h_samp_factor),
                i64::from(self.max_h_samp_factor * data_unit),
            ) as u32;
            self.comp_info[ci].height_in_blocks = jdiv_round_up(
                i64::from(self.image_height) * i64::from(c.v_samp_factor),
                i64::from(self.max_v_samp_factor * data_unit),
            ) as u32;
            self.master.first_mcu_col[ci] = 0;
            self.master.last_mcu_col[ci] = self.comp_info[ci].width_in_blocks.wrapping_sub(1);
            let c = self.comp_info[ci];
            self.comp_info[ci].downsampled_width = jdiv_round_up(
                i64::from(self.image_width) * i64::from(c.h_samp_factor),
                i64::from(self.max_h_samp_factor),
            ) as u32;
            self.comp_info[ci].downsampled_height = jdiv_round_up(
                i64::from(self.image_height) * i64::from(c.v_samp_factor),
                i64::from(self.max_v_samp_factor),
            ) as u32;
            self.comp_info[ci].component_needed = true;
            self.comp_info[ci].quant_table_latched = false;
        }
        self.total_imcu_rows = jdiv_round_up(
            i64::from(self.image_height),
            i64::from(self.max_v_samp_factor * data_unit),
        ) as u32;
        self.inputctl.has_multiple_scans = self.comps_in_scan < self.num_components || self.progressive_mode;
        Ok(())
    }

    /// `per_scan_setup`: MCU geometry for the current scan.
    fn per_scan_setup(&mut self) -> Result<()> {
        let data_unit = DCTSIZE as i32;
        if self.comps_in_scan == 1 {
            // Noninterleaved (single-component) scan.
            let ci = self.cur_comp_info[0].ok_or(Error::Internal("scan component"))?;
            let c = &mut self.comp_info[ci];
            self.mcus_per_row = c.width_in_blocks;
            self.mcu_rows_in_scan = c.height_in_blocks;
            c.mcu_width = 1;
            c.mcu_height = 1;
            c.mcu_blocks = 1;
            c.mcu_sample_width = c.dct_h_scaled_size;
            c.last_col_width = 1;
            // For noninterleaved scans, it doesn't matter what MCU_height is, but we set it.
            let mut tmp = (c.height_in_blocks % c.v_samp_factor as u32) as i32;
            if tmp == 0 {
                tmp = c.v_samp_factor;
            }
            c.last_row_height = tmp;
            self.blocks_in_mcu = 1;
            self.mcu_membership[0] = 0;
        } else {
            // Interleaved (multi-component) scan.
            if self.comps_in_scan <= 0 || self.comps_in_scan as usize > MAX_COMPS_IN_SCAN {
                return Err(Error::BadSampling);
            }
            self.mcus_per_row = jdiv_round_up(
                i64::from(self.image_width),
                i64::from(self.max_h_samp_factor * data_unit),
            ) as u32;
            self.mcu_rows_in_scan = jdiv_round_up(
                i64::from(self.image_height),
                i64::from(self.max_v_samp_factor * data_unit),
            ) as u32;
            self.blocks_in_mcu = 0;
            for ci in 0..self.comps_in_scan as usize {
                let idx = self.cur_comp_info[ci].ok_or(Error::Internal("scan component"))?;
                let c = &mut self.comp_info[idx];
                c.mcu_width = c.h_samp_factor;
                c.mcu_height = c.v_samp_factor;
                c.mcu_blocks = c.mcu_width * c.mcu_height;
                c.mcu_sample_width = c.mcu_width * c.dct_h_scaled_size;
                let mut tmp = (c.width_in_blocks % c.mcu_width as u32) as i32;
                if tmp == 0 {
                    tmp = c.mcu_width;
                }
                c.last_col_width = tmp;
                let mut tmp = (c.height_in_blocks % c.mcu_height as u32) as i32;
                if tmp == 0 {
                    tmp = c.mcu_height;
                }
                c.last_row_height = tmp;
                let mut mcublks = c.mcu_blocks;
                if self.blocks_in_mcu + mcublks > D_MAX_BLOCKS_IN_MCU as i32 {
                    return Err(Error::BadSampling);
                }
                while mcublks > 0 {
                    self.mcu_membership[self.blocks_in_mcu as usize] = ci as i32;
                    self.blocks_in_mcu += 1;
                    mcublks -= 1;
                }
            }
        }
        Ok(())
    }

    /// `latch_quant_tables`: copies each component's quantization table at the first scan that
    /// uses it (libjpeg never refreshes a latched table).
    fn latch_quant_tables(&mut self) -> Result<()> {
        for ci in 0..self.comps_in_scan as usize {
            let idx = self.cur_comp_info[ci].ok_or(Error::Internal("scan component"))?;
            if self.comp_info[idx].quant_table_latched {
                continue;
            }
            let qtblno = self.comp_info[idx].quant_tbl_no;
            if qtblno < 0
                || qtblno as usize >= crate::tables::NUM_QUANT_TBLS
                || self.quant_tbl_ptrs[qtblno as usize].is_none()
            {
                return Err(Error::Internal("missing quantization table"));
            }
            let qtbl = self.quant_tbl_ptrs[qtblno as usize].ok_or(Error::Internal("quant table"))?;
            self.comp_info[idx].quant_table = Some(qtbl.quantval);
            self.comp_info[idx].quant_table_latched = true;
        }
        Ok(())
    }

    /// `start_input_pass`: per-scan setup, then the entropy and coefficient passes.
    pub(crate) fn start_input_pass(&mut self) -> Result<()> {
        self.per_scan_setup()?;
        self.latch_quant_tables()?;
        self.start_pass_entropy()?;
        self.start_pass_coef_input()?;
        self.inputctl.consume = ConsumeKind::CoefData;
        Ok(())
    }

    /// `finish_input_pass`: back to reading markers.
    pub(crate) fn finish_input_pass(&mut self) {
        self.inputctl.consume = ConsumeKind::Markers;
    }

    /// `default_decompress_parms` (jdapimin.c): the colour spaces implied by the header.
    pub(crate) fn default_decompress_parms(&mut self) -> Result<()> {
        match self.num_components {
            1 => {
                self.jpeg_color_space = ColorSpace::Grayscale;
                self.out_color_space = ColorSpace::Grayscale;
            }
            3 => {
                if self.saw_jfif_marker {
                    self.jpeg_color_space = ColorSpace::YCbCr;
                } else if self.saw_adobe_marker {
                    match self.adobe_transform {
                        0 => self.jpeg_color_space = ColorSpace::Rgb,
                        1 => self.jpeg_color_space = ColorSpace::YCbCr,
                        _ => {
                            self.warn(crate::marker::JWRN_ADOBE_XFORM);
                            self.jpeg_color_space = ColorSpace::YCbCr;
                        }
                    }
                } else {
                    // No JFIF or Adobe marker: guess from the component identifiers.
                    let cid0 = self.comp_info[0].component_id;
                    let cid1 = self.comp_info[1].component_id;
                    let cid2 = self.comp_info[2].component_id;
                    if cid0 == 1 && cid1 == 2 && cid2 == 3 {
                        self.jpeg_color_space = ColorSpace::YCbCr;
                    } else if cid0 == 82 && cid1 == 71 && cid2 == 66 {
                        self.jpeg_color_space = ColorSpace::Rgb;
                    } else {
                        self.jpeg_color_space = ColorSpace::YCbCr;
                    }
                }
                self.out_color_space = ColorSpace::Rgb;
            }
            4 => {
                if self.saw_adobe_marker {
                    match self.adobe_transform {
                        0 => self.jpeg_color_space = ColorSpace::Cmyk,
                        2 => self.jpeg_color_space = ColorSpace::Ycck,
                        _ => {
                            self.warn(crate::marker::JWRN_ADOBE_XFORM);
                            self.jpeg_color_space = ColorSpace::Ycck;
                        }
                    }
                } else {
                    self.jpeg_color_space = ColorSpace::Cmyk;
                }
                self.out_color_space = ColorSpace::Cmyk;
            }
            _ => {
                self.jpeg_color_space = ColorSpace::Unknown;
                self.out_color_space = ColorSpace::Unknown;
            }
        }
        // Set defaults for other decompression parameters.
        self.scale_num = 1;
        self.scale_denom = 1;
        self.buffered_image = false;
        self.raw_data_out = false;
        self.dct_method = crate::decompress::DctMethod::IsLow;
        self.do_fancy_upsampling = true;
        self.do_block_smoothing = true;
        self.dither_mode = crate::decompress::DitherMode::FloydSteinberg;
        Ok(())
    }
}

/// `jdiv_round_up(a, b)` = `(a + b - 1) / b`.
pub(crate) fn jdiv_round_up(a: i64, b: i64) -> i64 {
    (a + b - 1) / b
}

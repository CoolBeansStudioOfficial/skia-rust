// Port of: libjpeg-turbo src/jdmaster.c#L34-L893 (libjpeg_turbo@e14cbfaa, 3.1.0)
//
// Copyright (C) 1991-1997, Thomas G. Lane. libjpeg-turbo Modifications: Copyright (C) 2002-2009,
// 2017, 2019, 2022-2023, D. R. Commander. Rust port Copyright (C) 2025 The skia-rust Authors.
// Licence: IJG (see LICENSE).
//
//! The master decompression control: output dimensions and scale selection, controller set-up
//! for the image, and the per-pass start-up of each controller.
//!
//! Quantization (`quantize_colors`, the colormap passes), merged upsampling and lossless are not
//! reached by Skia's use of libjpeg and are reported as `NotImplemented` when a call would need
//! them.

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

use crate::Decompress;
use crate::decompress::{DctMethod, GlobalState};
use crate::error::{Error, Result};
use crate::input::jdiv_round_up;
use crate::tables::{ColorSpace, DCTSIZE, DCTSIZE2, MAX_COMPONENTS, rgb_pixelsize};

/// `my_master_decompress` (the fields libjpeg keeps in the master controller).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MasterState {
    /// `first_MCU_col[ci]`.
    pub(crate) first_mcu_col: [u32; MAX_COMPONENTS],
    /// `last_MCU_col[ci]`.
    pub(crate) last_mcu_col: [u32; MAX_COMPONENTS],
    /// `first_iMCU_col`.
    pub(crate) first_imcu_col: u32,
    /// `last_iMCU_col`.
    pub(crate) last_imcu_col: u32,
    /// `last_good_iMCU_row`.
    pub(crate) last_good_imcu_row: u32,
    /// `jinit_upsampler_no_alloc`: set while `jpeg_crop_scanline` rebuilds the upsampler.
    pub(crate) jinit_upsampler_no_alloc: bool,
}

impl Default for MasterState {
    fn default() -> Self {
        MasterState {
            first_mcu_col: [0; MAX_COMPONENTS],
            last_mcu_col: [0; MAX_COMPONENTS],
            first_imcu_col: 0,
            last_imcu_col: 0,
            last_good_imcu_row: 0,
            jinit_upsampler_no_alloc: false,
        }
    }
}

impl Decompress {
    /// `jpeg_core_output_dimensions`: output size for `scale_num / scale_denom`, and the
    /// minimum scaled DCT size (`min_DCT_scaled_size`).
    pub(crate) fn core_output_dimensions(&mut self) {
        let sn = i64::from(self.scale_num);
        let sd = i64::from(self.scale_denom);
        let dct = DCTSIZE as i64;
        // Choose k in 1..=16 such that scale_num * DCTSIZE <= scale_denom * k (the first match).
        let mut k: i64 = 16;
        for cand in 1..=16i64 {
            if sn * dct <= sd * cand {
                k = cand;
                break;
            }
        }
        let w = i64::from(self.image_width);
        let h = i64::from(self.image_height);
        self.output_width = jdiv_round_up(w * k, dct) as u32;
        self.output_height = jdiv_round_up(h * k, dct) as u32;
        self.min_dct_scaled_size = k as i32;
        for ci in 0..self.num_components as usize {
            self.comp_info[ci].dct_h_scaled_size = self.min_dct_scaled_size;
            self.comp_info[ci].dct_v_scaled_size = self.min_dct_scaled_size;
        }
    }

    /// `jpeg_calc_output_dimensions` after `jpeg_core_output_dimensions`: per-component
    /// scaled DCT sizes and downsampled sizes.
    pub(crate) fn calc_output_dimensions_full(&mut self) -> Result<()> {
        self.core_output_dimensions();
        let dct = DCTSIZE as i32;
        for ci in 0..self.num_components as usize {
            let mut ssize = self.min_dct_scaled_size;
            while ssize < dct
                && ((self.max_h_samp_factor * self.min_dct_scaled_size)
                    % (self.comp_info[ci].h_samp_factor * ssize * 2))
                    == 0
                && ((self.max_v_samp_factor * self.min_dct_scaled_size)
                    % (self.comp_info[ci].v_samp_factor * ssize * 2))
                    == 0
            {
                ssize *= 2;
            }
            self.comp_info[ci].dct_h_scaled_size = ssize;
            self.comp_info[ci].dct_v_scaled_size = ssize;
        }
        for ci in 0..self.num_components as usize {
            let c = self.comp_info[ci];
            self.comp_info[ci].downsampled_width = jdiv_round_up(
                i64::from(self.image_width) * i64::from(c.h_samp_factor * c.dct_h_scaled_size),
                i64::from(self.max_h_samp_factor * dct),
            ) as u32;
            self.comp_info[ci].downsampled_height = jdiv_round_up(
                i64::from(self.image_height) * i64::from(c.v_samp_factor * c.dct_v_scaled_size),
                i64::from(self.max_v_samp_factor * dct),
            ) as u32;
        }
        self.calc_output_components();
        Ok(())
    }

    /// The `out_color_components` / `rec_outbuf_height` part of `jpeg_calc_output_dimensions`.
    pub(crate) fn calc_output_components(&mut self) {
        self.out_color_components = match self.out_color_space {
            ColorSpace::Grayscale => 1,
            ColorSpace::Rgb
            | ColorSpace::ExtRgb
            | ColorSpace::ExtRgbx
            | ColorSpace::ExtBgr
            | ColorSpace::ExtBgrx
            | ColorSpace::ExtXbgr
            | ColorSpace::ExtXrgb
            | ColorSpace::ExtRgba
            | ColorSpace::ExtBgra
            | ColorSpace::ExtAbgr
            | ColorSpace::ExtArgb => rgb_pixelsize(self.out_color_space),
            ColorSpace::YCbCr | ColorSpace::Rgb565 => 3,
            ColorSpace::Cmyk | ColorSpace::Ycck => 4,
            ColorSpace::Unknown => self.num_components,
        };
        self.output_components = self.out_color_components;
        self.rec_outbuf_height = 1;
    }

    /// `jinit_master_decompress` / `master_selection`: builds the controllers for the image and
    /// starts the first input pass.
    pub(crate) fn jinit_master_decompress(&mut self) -> Result<()> {
        // jpeg_calc_output_dimensions
        if self.global_state != GlobalState::Ready {
            return Err(Error::BadState(self.global_state as i32));
        }
        self.calc_output_dimensions_full()?;
        // Merged upsampling (jdmerge.c) is only used with do_fancy_upsampling off, which Skia
        // never sets; refuse the combination rather than silently using the wrong path.
        if self.use_merged_upsample() {
            return Err(Error::NotImplemented);
        }
        if self.raw_data_out && self.data_precision != 8 {
            return Err(Error::BadPrecision);
        }
        if !self.raw_data_out {
            self.jinit_color_deconverter()?;
            self.jinit_upsampler()?;
        }
        self.jinit_entropy_decoder()?;
        self.jinit_inverse_dct()?;
        self.jinit_d_coef_controller()?;
        if !self.raw_data_out {
            self.jinit_d_main_controller()?;
        }
        // Initialize the input controller's per-image MCU bookkeeping for the first scan.
        self.start_input_pass_master()?;
        self.master.first_imcu_col = 0;
        self.master.last_imcu_col = self.mcus_per_row.wrapping_sub(1);
        self.master.last_good_imcu_row = 0;
        Ok(())
    }

    /// `use_merged_upsample`.
    fn use_merged_upsample(&self) -> bool {
        if self.do_fancy_upsampling || self.ccir601_sampling {
            return false;
        }
        // Merged upsampling is not ported; if it would apply, the caller reports NotImplemented.
        if self.jpeg_color_space != ColorSpace::YCbCr || self.num_components != 3 {
            return false;
        }
        matches!(
            self.out_color_space,
            ColorSpace::Rgb
                | ColorSpace::Rgb565
                | ColorSpace::ExtRgb
                | ColorSpace::ExtRgbx
                | ColorSpace::ExtBgr
                | ColorSpace::ExtBgrx
                | ColorSpace::ExtXbgr
                | ColorSpace::ExtXrgb
                | ColorSpace::ExtRgba
                | ColorSpace::ExtBgra
                | ColorSpace::ExtAbgr
                | ColorSpace::ExtArgb
        ) && self.comp_info.len() >= 3
            && self.comp_info[0].h_samp_factor == 2
            && self.comp_info[1].h_samp_factor == 1
            && self.comp_info[2].h_samp_factor == 1
            && self.comp_info[0].v_samp_factor <= 2
            && self.comp_info[1].v_samp_factor == 1
            && self.comp_info[2].v_samp_factor == 1
    }

    /// `start_input_pass` as called by `master_selection`.
    fn start_input_pass_master(&mut self) -> Result<()> {
        self.start_input_pass()
    }

    /// `prepare_for_output_pass` (the non-quantizing path).
    pub(crate) fn prepare_for_output_pass(&mut self) -> Result<()> {
        self.start_pass_idct()?;
        self.start_output_pass_coef()?;
        if !self.raw_data_out {
            self.start_pass_color_convert()?;
            self.start_pass_upsample()?;
            self.start_pass_main()?;
        }
        Ok(())
    }

    /// `finish_output_pass`: nothing to do without quantization.
    pub(crate) fn finish_output_pass(&mut self) -> Result<()> {
        Ok(())
    }

    /// `jinit_inverse_dct` / `start_pass` of `jddctmgr.c`: the method per component.
    pub(crate) fn jinit_inverse_dct(&mut self) -> Result<()> {
        self.idct_size = vec![0; self.num_components as usize];
        Ok(())
    }

    /// `jddctmgr.c` `start_pass`: chooses the IDCT size per component and builds the
    /// dequantization multipliers (natural order) once per image.
    pub(crate) fn start_pass_idct(&mut self) -> Result<()> {
        if self.dct_method != DctMethod::IsLow {
            return Err(Error::NotImplemented);
        }
        self.dct_tables = vec![[0i32; 64]; self.num_components as usize];
        for ci in 0..self.num_components as usize {
            let c = self.comp_info[ci];
            let size = c.dct_h_scaled_size;
            if !(1..=16).contains(&size) {
                return Err(Error::BadDctSize);
            }
            self.idct_size[ci] = size;
            if !c.component_needed {
                continue;
            }
            if let Some(q) = c.quant_table {
                for i in 0..64 {
                    self.dct_tables[ci][i] = i32::from(q[i]);
                }
            }
        }
        Ok(())
    }

    /// `jinit_huff_decoder`: only sequential Huffman is ported. Progressive and arithmetic
    /// decoding are reported as not implemented.
    pub(crate) fn jinit_entropy_decoder(&mut self) -> Result<()> {
        if self.arith_code {
            return Err(Error::ArithNotImplemented);
        }
        if self.progressive_mode {
            // jinit_phuff_decoder: every coefficient starts at bit position -1 (not yet coded).
            let nc = self.num_components as usize;
            self.coef_bits = vec![[-1i32; DCTSIZE2]; 2 * nc];
        }
        Ok(())
    }

    /// `start_pass` of the entropy decoder.
    pub(crate) fn start_pass_entropy(&mut self) -> Result<()> {
        if self.progressive_mode {
            return self.start_pass_phuff_decoder();
        }
        self.start_pass_huff_decoder()
    }

    /// `jinit_d_coef_controller` / `jinit_d_main_controller` allocations are sized on demand.
    pub(crate) fn jinit_d_coef_controller(&mut self) -> Result<()> {
        // The whole image is buffered for multi-scan input and for buffered-image mode.
        self.coef_buffered = self.inputctl.has_multiple_scans || self.buffered_image;
        if self.coef_buffered {
            self.alloc_whole_image();
        }
        Ok(())
    }

    /// `jinit_d_main_controller` (the non-context case is always used for the baseline path,
    /// and the context case when `need_context_rows` is set by the upsampler).
    pub(crate) fn jinit_d_main_controller(&mut self) -> Result<()> {
        if self.data_precision != 8 {
            return Err(Error::BadPrecision);
        }
        self.init_main_buffers()
    }
}

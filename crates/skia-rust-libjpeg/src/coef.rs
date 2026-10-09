// Port of: libjpeg-turbo src/jdcoefct.c#L29-L169 and #L816-L885 (libjpeg_turbo@e14cbfaa, 3.1.0)
//
// Copyright (C) 1994-1997, Thomas G. Lane. libjpeg-turbo Modifications: Copyright (C) 2010-2011,
// 2015-2016, D. R. Commander. Rust port Copyright (C) 2025 The skia-rust Authors. Licence: IJG.
//
//! The coefficient buffer controller for single-pass (baseline, non-buffered) decoding:
//! `decompress_onepass`. Each MCU is decoded into a small block buffer and inverse-transformed
//! straight into the sample rows.
//!
//! Multi-pass decoding (progressive input, buffered-image mode, block smoothing) needs the whole
//! image's coefficients and is not ported; `master.rs` reports it as `NotImplemented`.

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
use crate::error::{Error, Result};
use crate::idct::{RangeLimit, idct_for_size};
use crate::marker::ConsumeResult;
use crate::tables::{D_MAX_BLOCKS_IN_MCU, DCTSIZE2};

/// `my_coef_controller` (the single-pass fields).
#[derive(Debug, Clone)]
pub(crate) struct CoefState {
    /// `MCU_ctr`: the next MCU column to process (resume point).
    pub(crate) mcu_ctr: u32,
    /// `MCU_vert_offset`: the next MCU row within the iMCU row (resume point).
    pub(crate) mcu_vert_offset: i32,
    /// `MCU_rows_per_iMCU_row`.
    pub(crate) mcu_rows_per_imcu_row: i32,
    /// `MCU_buffer[]`: the blocks of one MCU.
    pub(crate) mcu_buffer: [[i16; DCTSIZE2]; D_MAX_BLOCKS_IN_MCU],
}

impl Default for CoefState {
    fn default() -> Self {
        CoefState {
            mcu_ctr: 0,
            mcu_vert_offset: 0,
            mcu_rows_per_imcu_row: 0,
            mcu_buffer: [[0; DCTSIZE2]; D_MAX_BLOCKS_IN_MCU],
        }
    }
}

/// Output rows for one pass: per component, the storage of sample rows and the list of row
/// indices (`output_buf[ci]`), with index 0 being the first row of the pass.
pub(crate) struct OutPlanes<'a> {
    /// Row storage, one entry per component.
    pub(crate) storage: &'a mut [Vec<Vec<u8>>],
    /// Row index lists, one per component.
    pub(crate) lists: &'a [Vec<usize>],
}

impl Decompress {
    /// `start_iMCU_row`.
    pub(crate) fn start_imcu_row(&mut self) {
        if self.comps_in_scan > 1 {
            self.coef.mcu_rows_per_imcu_row = 1;
        } else {
            let ci = self.cur_comp_info[0].unwrap_or(0);
            if self.input_imcu_row + 1 < self.total_imcu_rows {
                self.coef.mcu_rows_per_imcu_row = self.comp_info[ci].v_samp_factor;
            } else {
                self.coef.mcu_rows_per_imcu_row = self.comp_info[ci].last_row_height;
            }
        }
        self.coef.mcu_ctr = 0;
        self.coef.mcu_vert_offset = 0;
    }

    /// `start_input_pass` of the coefficient controller.
    pub(crate) fn start_pass_coef_input(&mut self) -> Result<()> {
        self.input_imcu_row = 0;
        self.start_imcu_row();
        Ok(())
    }

    /// `start_output_pass` of the coefficient controller.
    pub(crate) fn start_output_pass_coef(&mut self) -> Result<()> {
        if self.coef_buffered {
            self.start_output_pass_buffered();
        }
        self.output_imcu_row = 0;
        Ok(())
    }

    /// `consume_data` for the single-pass path is `dummy_consume_data`, but a multi-scan image
    /// never reaches it (see `master.rs`). Kept as the dispatch target of `ConsumeKind::CoefData`.
    pub(crate) fn coef_consume_data(&mut self) -> Result<ConsumeResult> {
        if self.coef_buffered {
            return self.consume_data_buffered();
        }
        Ok(ConsumeResult::Suspended)
    }

    /// `_decompress_data` of the coefficient controller: the output routine for this pass.
    pub(crate) fn decompress_coef_output(
        &mut self,
        out: &mut OutPlanes<'_>,
    ) -> Result<Option<ConsumeResult>> {
        if !self.coef_buffered {
            return self.decompress_onepass(out);
        }
        if self.coef_smooth {
            self.decompress_smooth_data_buffered(out)
        } else {
            self.decompress_data_buffered(out)
        }
    }

    /// `decompress_onepass`: decodes and inverse-transforms MCUs of the current iMCU row into
    /// `out`. Returns `Ok(None)` on suspension, or `Ok(Some(code))` where code is `ROW_COMPLETED`
    /// or `SCAN_COMPLETED`.
    pub(crate) fn decompress_onepass(
        &mut self,
        out: &mut OutPlanes<'_>,
    ) -> Result<Option<ConsumeResult>> {
        let last_mcu_col = self.mcus_per_row.wrapping_sub(1);
        let last_imcu_row = self.total_imcu_rows.wrapping_sub(1);
        let range_limit = std::sync::Arc::clone(&self.range_limit);
        self.decompress_onepass_inner(out, &range_limit, last_mcu_col, last_imcu_row)
    }

    fn decompress_onepass_inner(
        &mut self,
        out: &mut OutPlanes<'_>,
        range_limit: &RangeLimit,
        last_mcu_col: u32,
        last_imcu_row: u32,
    ) -> Result<Option<ConsumeResult>> {
        let mut yoffset = self.coef.mcu_vert_offset;
        while yoffset < self.coef.mcu_rows_per_imcu_row {
            let mut mcu_col_num = self.coef.mcu_ctr;
            while mcu_col_num <= last_mcu_col {
                // jzero_far(MCU_buffer[0], blocks_in_MCU * sizeof(JBLOCK))
                for b in 0..self.blocks_in_mcu as usize {
                    self.coef.mcu_buffer[b] = [0; DCTSIZE2];
                }
                if !self.insufficient_data {
                    self.master.last_good_imcu_row = self.input_imcu_row;
                }
                let mut mcu = self.coef.mcu_buffer;
                if !self.decode_mcu(Some(&mut mcu[..]))? {
                    self.coef.mcu_vert_offset = yoffset;
                    self.coef.mcu_ctr = mcu_col_num;
                    return Ok(None);
                }
                self.coef.mcu_buffer = mcu;

                // Only do the inverse DCT when the MCU is inside the scanned range.
                if mcu_col_num >= self.master.first_imcu_col
                    && mcu_col_num <= self.master.last_imcu_col
                {
                    let mut blkn = 0usize;
                    for ci in 0..self.comps_in_scan as usize {
                        let comp_idx =
                            self.cur_comp_info[ci].ok_or(Error::Internal("scan component"))?;
                        let c = self.comp_info[comp_idx];
                        if !c.component_needed {
                            blkn += c.mcu_blocks as usize;
                            continue;
                        }
                        let comp_index = c.component_index as usize;
                        let dct = c.dct_h_scaled_size;
                        let useful_width = if mcu_col_num < last_mcu_col {
                            c.mcu_width
                        } else {
                            c.last_col_width
                        } as usize;
                        // output_ptr = output_buf[component_index] + yoffset * DCT_scaled_size
                        let mut out_row = yoffset as usize * dct as usize;
                        let start_col = (mcu_col_num - self.master.first_imcu_col) as usize
                            * c.mcu_sample_width as usize;
                        for yindex in 0..c.mcu_height as usize {
                            if self.input_imcu_row < last_imcu_row
                                || (yoffset as usize + yindex) < c.last_row_height as usize
                            {
                                let mut output_col = start_col;
                                for xindex in 0..useful_width {
                                    let block = &self.coef.mcu_buffer[blkn + xindex];
                                    let quant = &self.dct_tables[comp_index];
                                    let res = idct_for_size(dct, block, quant, range_limit)
                                        .ok_or(Error::BadDctSize)?;
                                    store_block(
                                        out,
                                        comp_index,
                                        out_row,
                                        output_col,
                                        dct as usize,
                                        &res,
                                    );
                                    output_col += dct as usize;
                                }
                            }
                            blkn += c.mcu_width as usize;
                            out_row += dct as usize;
                        }
                    }
                }
                mcu_col_num += 1;
            }
            self.coef.mcu_ctr = 0;
            yoffset += 1;
        }
        self.output_imcu_row += 1;
        self.input_imcu_row += 1;
        if self.input_imcu_row < self.total_imcu_rows {
            self.start_imcu_row();
            return Ok(Some(ConsumeResult::RowCompleted));
        }
        self.finish_input_pass();
        Ok(Some(ConsumeResult::ScanCompleted))
    }
}

/// Copies the `size x size` top-left of an IDCT result into the output rows (the
/// `output_buf[ci][row] + col` writes of `jidctint.c`).
pub(crate) fn store_block(
    out: &mut OutPlanes<'_>,
    comp: usize,
    row0: usize,
    col0: usize,
    size: usize,
    res: &[[u8; 16]; 16],
) {
    let list = &out.lists[comp];
    let storage = &mut out.storage[comp];
    for r in 0..size {
        let row_idx = list[row0 + r];
        let dst = &mut storage[row_idx];
        dst[col0..col0 + size].copy_from_slice(&res[r][..size]);
    }
}

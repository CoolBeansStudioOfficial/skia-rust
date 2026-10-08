// Port of: libjpeg-turbo src/jdcoefct.c#L29-L169, #L173-L302, #L313-L553 and #L816-L885
//          (libjpeg_turbo@e14cbfaa, 3.1.0)
//
// Copyright (C) 1994-1997, Thomas G. Lane. libjpeg-turbo Modifications: Copyright (C) 2010-2011,
// 2015-2016, 2019, D. R. Commander. Rust port Copyright (C) 2025 The skia-rust Authors. Licence: IJG.
//
//! The multi-pass coefficient controller: the whole image's coefficients are kept in memory
//! (`whole_image`, libjpeg's virtual block arrays), filled one scan at a time by `consume_data`,
//! and turned into sample rows by `decompress_data`, or by `decompress_smooth_data` when block
//! smoothing applies. Used for progressive input and for buffered-image mode.

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
use crate::coef::{OutPlanes, store_block};
use crate::error::{Error, Result};
use crate::idct::idct_for_size;
use crate::marker::ConsumeResult;
use crate::tables::{D_MAX_BLOCKS_IN_MCU, DCTSIZE2};

/// `SAVED_COEFS`: the coefficient bit positions saved for smoothing (`coef_bits[0..9]`).
pub(crate) const SAVED_COEFS: usize = 10;

/// One component's `whole_image` virtual array: `stride` blocks per row, padded to a multiple of
/// the sampling factors (`jround_up`), and as many block rows as the padded height.
#[derive(Debug, Clone, Default)]
pub(crate) struct CompCoefs {
    /// Blocks per row (`width_in_blocks` rounded up to `h_samp_factor`).
    pub(crate) stride: usize,
    /// The blocks, row by row.
    pub(crate) data: Vec<[i16; DCTSIZE2]>,
}

impl CompCoefs {
    /// Index of block `(row, col)`.
    #[inline]
    fn index(&self, row: usize, col: usize) -> usize {
        row * self.stride + col
    }

    /// The DC coefficient of block `(row, col)`, as `(int)buffer[row][col][0]`.
    #[inline]
    fn dc(&self, row: usize, col: usize) -> i32 {
        i32::from(self.data[self.index(row, col)][0])
    }
}

/// `smooth_pred`: the `pred` computation of `decompress_smooth_data`, for one coefficient.
/// `clamp` applies the `Al` limit (the DC estimate does not clamp).
#[inline]
fn smooth_pred(num: i32, q: i32, al: i32, clamp: bool) -> i32 {
    if num >= 0 {
        let mut pred = q.wrapping_shl(7).wrapping_add(num) / q.wrapping_shl(8);
        if clamp && al > 0 && pred >= (1 << al) {
            pred = (1 << al) - 1;
        }
        pred
    } else {
        let mut pred = q.wrapping_shl(7).wrapping_sub(num) / q.wrapping_shl(8);
        if clamp && al > 0 && pred >= (1 << al) {
            pred = (1 << al) - 1;
        }
        pred.wrapping_neg()
    }
}

/// `Q01_POS` .. `Q30_POS`: the zigzag-position indexes of the quantization table that the
/// smoothing estimates read (`jdcoefct.c#L330-L338`).
const Q01_POS: usize = 1;
const Q10_POS: usize = 8;
const Q20_POS: usize = 16;
const Q11_POS: usize = 9;
const Q02_POS: usize = 2;
const Q03_POS: usize = 3;
const Q12_POS: usize = 10;
const Q21_POS: usize = 17;
const Q30_POS: usize = 24;

impl Decompress {
    /// Allocates the whole-image arrays (`jinit_d_coef_controller` with `need_full_buffer`).
    pub(crate) fn alloc_whole_image(&mut self) {
        self.whole_image = (0..self.num_components as usize)
            .map(|ci| {
                let c = self.comp_info[ci];
                let hs = c.h_samp_factor as usize;
                let vs = c.v_samp_factor as usize;
                let stride = (c.width_in_blocks as usize).div_ceil(hs) * hs;
                let rows = (c.height_in_blocks as usize).div_ceil(vs) * vs;
                CompCoefs {
                    stride,
                    data: vec![[0i16; DCTSIZE2]; stride * rows],
                }
            })
            .collect();
    }

    /// `start_output_pass` of the buffered controller: picks block smoothing when it applies.
    pub(crate) fn start_output_pass_buffered(&mut self) {
        self.coef_smooth = self.do_block_smoothing && self.smoothing_ok();
    }

    /// `smoothing_ok`: whether block smoothing is useful for this progressive image, and latches
    /// the coefficient bit positions it uses.
    fn smoothing_ok(&mut self) -> bool {
        if !self.progressive_mode || self.coef_bits.is_empty() {
            return false;
        }
        let nc = self.num_components as usize;
        if self.coef_bits_latch.len() != 2 * nc {
            self.coef_bits_latch = vec![[0i32; SAVED_COEFS]; 2 * nc];
        }
        let mut smoothing_useful = false;
        for ci in 0..nc {
            let Some(qtable) = self.comp_info[ci].quant_table else {
                return false;
            };
            let q = |pos: usize| i32::from(qtable[pos]);
            if q(0) == 0
                || q(Q01_POS) == 0
                || q(Q10_POS) == 0
                || q(Q20_POS) == 0
                || q(Q11_POS) == 0
                || q(Q02_POS) == 0
                || q(Q03_POS) == 0
                || q(Q12_POS) == 0
                || q(Q21_POS) == 0
                || q(Q30_POS) == 0
            {
                return false;
            }
            let coef_bits = self.coef_bits[ci];
            let prev_coef_bits = self.coef_bits[ci + nc];
            if coef_bits[0] < 0 {
                return false;
            }
            self.coef_bits_latch[ci][0] = coef_bits[0];
            for coefi in 1..SAVED_COEFS {
                if self.input_scan_number > 1 {
                    self.coef_bits_latch[ci + nc][coefi] = prev_coef_bits[coefi];
                } else {
                    self.coef_bits_latch[ci + nc][coefi] = -1;
                }
                self.coef_bits_latch[ci][coefi] = coef_bits[coefi];
                if coef_bits[coefi] != 0 {
                    smoothing_useful = true;
                }
            }
        }
        smoothing_useful
    }

    /// `consume_data`: decodes the MCUs of the current scan into the whole-image arrays.
    pub(crate) fn consume_data_buffered(&mut self) -> Result<ConsumeResult> {
        let nscan = self.comps_in_scan as usize;
        let mut yoffset = self.coef.mcu_vert_offset;
        while yoffset < self.coef.mcu_rows_per_imcu_row {
            let mut mcu_col_num = self.coef.mcu_ctr;
            while mcu_col_num < self.mcus_per_row {
                // The blocks of this MCU, as pointers into the whole-image arrays (here copies,
                // written back below whether or not the decode suspends).
                let mut mcu = [[0i16; DCTSIZE2]; D_MAX_BLOCKS_IN_MCU];
                let mut targets: Vec<(usize, usize, usize)> =
                    Vec::with_capacity(self.blocks_in_mcu as usize);
                let mut blkn = 0usize;
                for ci in 0..nscan {
                    let compptr = self.comp_info
                        [self.cur_comp_info[ci].ok_or(Error::Internal("scan component"))?];
                    let comp_index = compptr.component_index as usize;
                    let start_col = mcu_col_num as usize * compptr.mcu_width as usize;
                    for yindex in 0..compptr.mcu_height as usize {
                        let row = self.input_imcu_row as usize * compptr.v_samp_factor as usize
                            + yindex
                            + yoffset as usize;
                        for xindex in 0..compptr.mcu_width as usize {
                            let col = start_col + xindex;
                            let whole = &self.whole_image[comp_index];
                            mcu[blkn] = whole.data[whole.index(row, col)];
                            targets.push((comp_index, row, col));
                            blkn += 1;
                        }
                    }
                }
                if !self.insufficient_data {
                    self.master.last_good_imcu_row = self.input_imcu_row;
                }
                let ok = self.decode_entropy_mcu(&mut mcu[..blkn])?;
                for (b, &(comp_index, row, col)) in targets.iter().enumerate() {
                    let whole = &mut self.whole_image[comp_index];
                    let idx = whole.index(row, col);
                    whole.data[idx] = mcu[b];
                }
                if !ok {
                    self.coef.mcu_vert_offset = yoffset;
                    self.coef.mcu_ctr = mcu_col_num;
                    return Ok(ConsumeResult::Suspended);
                }
                mcu_col_num += 1;
            }
            self.coef.mcu_ctr = 0;
            yoffset += 1;
        }
        self.input_imcu_row += 1;
        if self.input_imcu_row < self.total_imcu_rows {
            self.start_imcu_row();
            return Ok(ConsumeResult::RowCompleted);
        }
        self.finish_input_pass();
        Ok(ConsumeResult::ScanCompleted)
    }

    /// `decompress_data`: inverse-transforms one iMCU row of the whole-image arrays into the
    /// output rows. `None` is a suspension.
    pub(crate) fn decompress_data_buffered(
        &mut self,
        out: &mut OutPlanes<'_>,
    ) -> Result<Option<ConsumeResult>> {
        while self.input_scan_number < self.output_scan_number
            || (self.input_scan_number == self.output_scan_number
                && self.input_imcu_row <= self.output_imcu_row)
        {
            if self.inputctl_consume()? == ConsumeResult::Suspended {
                return Ok(None);
            }
        }
        let last_imcu_row = self.total_imcu_rows.wrapping_sub(1);
        let range_limit = std::rc::Rc::clone(&self.range_limit);
        for ci in 0..self.num_components as usize {
            let compptr = self.comp_info[ci];
            if !compptr.component_needed {
                continue;
            }
            let vs = compptr.v_samp_factor as u32;
            let block_rows = if self.output_imcu_row < last_imcu_row {
                vs
            } else {
                let r = compptr.height_in_blocks % vs;
                if r == 0 { vs } else { r }
            };
            let dct = compptr.dct_h_scaled_size as usize;
            for block_row in 0..block_rows as usize {
                let row = self.output_imcu_row as usize * vs as usize + block_row;
                let mut output_col = 0usize;
                for block_num in self.master.first_mcu_col[ci]..=self.master.last_mcu_col[ci] {
                    let block = self.whole_image[ci].block_at(row, block_num as usize);
                    let res = idct_for_size(dct as i32, &block, &self.dct_tables[ci], &range_limit)
                        .ok_or(Error::BadDctSize)?;
                    store_block(out, ci, block_row * dct, output_col, dct, &res);
                    output_col += dct;
                }
            }
        }
        self.output_imcu_row += 1;
        if self.output_imcu_row < self.total_imcu_rows {
            return Ok(Some(ConsumeResult::RowCompleted));
        }
        Ok(Some(ConsumeResult::ScanCompleted))
    }

    /// `decompress_smooth_data`: as `decompress_data`, but each block's zero AC and DC
    /// coefficients are estimated from its neighbours first (`jdcoefct.c#L553-L785`).
    pub(crate) fn decompress_smooth_data_buffered(
        &mut self,
        out: &mut OutPlanes<'_>,
    ) -> Result<Option<ConsumeResult>> {
        let last_imcu_row = self.total_imcu_rows.wrapping_sub(1);
        while self.input_scan_number <= self.output_scan_number && !self.inputctl.eoi_reached {
            if self.input_scan_number == self.output_scan_number {
                let delta = if self.ss == 0 { 2 } else { 0 };
                if self.input_imcu_row > self.output_imcu_row + delta {
                    break;
                }
            }
            if self.inputctl_consume()? == ConsumeResult::Suspended {
                return Ok(None);
            }
        }
        let range_limit = std::rc::Rc::clone(&self.range_limit);
        for ci in 0..self.num_components as usize {
            let compptr = self.comp_info[ci];
            if !compptr.component_needed {
                continue;
            }
            let vs = compptr.v_samp_factor as u32;
            let oi = self.output_imcu_row;
            let block_rows = if oi < last_imcu_row {
                vs
            } else {
                let r = compptr.height_in_blocks % vs;
                if r == 0 { vs } else { r }
            };
            let coef_bits: [i32; SAVED_COEFS] =
                if self.output_imcu_row > self.master.last_good_imcu_row {
                    self.coef_bits_latch[ci + self.num_components as usize]
                } else {
                    self.coef_bits_latch[ci]
                };
            let change_dc = coef_bits[1] == -1
                && coef_bits[2] == -1
                && coef_bits[3] == -1
                && coef_bits[4] == -1
                && coef_bits[5] == -1
                && coef_bits[6] == -1
                && coef_bits[7] == -1
                && coef_bits[8] == -1
                && coef_bits[9] == -1;
            let qt = self.comp_info[ci]
                .quant_table
                .ok_or(Error::Internal("quant table"))?;
            let q00 = i32::from(qt[0]);
            let q01 = i32::from(qt[Q01_POS]);
            let q10 = i32::from(qt[Q10_POS]);
            let q20 = i32::from(qt[Q20_POS]);
            let q11 = i32::from(qt[Q11_POS]);
            let q02 = i32::from(qt[Q02_POS]);
            let (mut q03, mut q12, mut q21, mut q30) = (0i32, 0i32, 0i32, 0i32);
            if change_dc {
                q03 = i32::from(qt[Q03_POS]);
                q12 = i32::from(qt[Q12_POS]);
                q21 = i32::from(qt[Q21_POS]);
                q30 = i32::from(qt[Q30_POS]);
            }
            let dct = compptr.dct_h_scaled_size as usize;
            let image_block_rows = block_rows as usize * self.total_imcu_rows as usize;
            let first = self.master.first_mcu_col[ci] as usize;
            let last = self.master.last_mcu_col[ci] as usize;
            let last_block_column = compptr.width_in_blocks as usize - 1;
            for block_row in 0..block_rows as usize {
                // `image_block_row` is libjpeg's edge test; `cur` is the block row in the array
                // (`buffer[block_row]`). They differ only in the last iMCU row, as in the C code.
                let image_block_row = oi as usize * block_rows as usize + block_row;
                let whole = &self.whole_image[ci];
                let cur = oi as usize * vs as usize + block_row;
                let prev = if image_block_row > 0 { cur - 1 } else { cur };
                let prev_prev = if image_block_row > 1 { cur - 2 } else { prev };
                let next = if image_block_row < image_block_rows - 1 {
                    cur + 1
                } else {
                    cur
                };
                let next_next = if image_block_row < image_block_rows - 2 {
                    cur + 2
                } else {
                    next
                };
                let mut dc = [0i32; 26];
                let mut output_col = 0usize;
                // DC01..DC05 = prev_prev, DC06..DC10 = prev, DC11..DC15 = cur, DC16..DC20 = next,
                // DC21..DC25 = next_next, all at the first block column.
                for k in 1..=5 {
                    dc[k] = whole.dc(prev_prev, first);
                    dc[k + 5] = whole.dc(prev, first);
                    dc[k + 10] = whole.dc(cur, first);
                    dc[k + 15] = whole.dc(next, first);
                    dc[k + 20] = whole.dc(next_next, first);
                }
                for block_num in first..=last {
                    let mut workspace = whole.block_at(cur, block_num);
                    if block_num == first && block_num < last_block_column {
                        dc[4] = whole.dc(prev_prev, block_num + 1);
                        dc[5] = dc[4];
                        dc[9] = whole.dc(prev, block_num + 1);
                        dc[10] = dc[9];
                        dc[14] = whole.dc(cur, block_num + 1);
                        dc[15] = dc[14];
                        dc[19] = whole.dc(next, block_num + 1);
                        dc[20] = dc[19];
                        dc[24] = whole.dc(next_next, block_num + 1);
                        dc[25] = dc[24];
                    }
                    if block_num + 1 < last_block_column {
                        dc[5] = whole.dc(prev_prev, block_num + 2);
                        dc[10] = whole.dc(prev, block_num + 2);
                        dc[15] = whole.dc(cur, block_num + 2);
                        dc[20] = whole.dc(next, block_num + 2);
                        dc[25] = whole.dc(next_next, block_num + 2);
                    }
                    // coef_bits[1] / workspace[1] (q01)
                    if coef_bits[1] != 0 && workspace[1] == 0 {
                        let num = q00.wrapping_mul(if change_dc {
                            dc[1]
                                .wrapping_neg()
                                .wrapping_sub(dc[2])
                                .wrapping_add(dc[4])
                                .wrapping_add(dc[5])
                                .wrapping_sub(3i32.wrapping_mul(dc[6]))
                                .wrapping_add(13i32.wrapping_mul(dc[7]))
                                .wrapping_sub(13i32.wrapping_mul(dc[9]))
                                .wrapping_add(3i32.wrapping_mul(dc[10]))
                                .wrapping_sub(3i32.wrapping_mul(dc[11]))
                                .wrapping_add(38i32.wrapping_mul(dc[12]))
                                .wrapping_sub(38i32.wrapping_mul(dc[14]))
                                .wrapping_add(3i32.wrapping_mul(dc[15]))
                                .wrapping_sub(3i32.wrapping_mul(dc[16]))
                                .wrapping_add(13i32.wrapping_mul(dc[17]))
                                .wrapping_sub(13i32.wrapping_mul(dc[19]))
                                .wrapping_add(3i32.wrapping_mul(dc[20]))
                                .wrapping_sub(dc[21])
                                .wrapping_sub(dc[22])
                                .wrapping_add(dc[24])
                                .wrapping_add(dc[25])
                        } else {
                            7i32.wrapping_neg()
                                .wrapping_mul(dc[11])
                                .wrapping_add(50i32.wrapping_mul(dc[12]))
                                .wrapping_sub(50i32.wrapping_mul(dc[14]))
                                .wrapping_add(7i32.wrapping_mul(dc[15]))
                        });
                        workspace[1] = smooth_pred(num, q01, coef_bits[1], true) as i16;
                    }
                    // coef_bits[2] / workspace[8] (q10)
                    if coef_bits[2] != 0 && workspace[8] == 0 {
                        let num = q00.wrapping_mul(if change_dc {
                            dc[1]
                                .wrapping_neg()
                                .wrapping_sub(3i32.wrapping_mul(dc[2]))
                                .wrapping_sub(3i32.wrapping_mul(dc[3]))
                                .wrapping_sub(3i32.wrapping_mul(dc[4]))
                                .wrapping_sub(dc[5])
                                .wrapping_sub(dc[6])
                                .wrapping_add(13i32.wrapping_mul(dc[7]))
                                .wrapping_add(38i32.wrapping_mul(dc[8]))
                                .wrapping_add(13i32.wrapping_mul(dc[9]))
                                .wrapping_sub(dc[10])
                                .wrapping_add(dc[16])
                                .wrapping_sub(13i32.wrapping_mul(dc[17]))
                                .wrapping_sub(38i32.wrapping_mul(dc[18]))
                                .wrapping_sub(13i32.wrapping_mul(dc[19]))
                                .wrapping_add(dc[20])
                                .wrapping_add(dc[21])
                                .wrapping_add(3i32.wrapping_mul(dc[22]))
                                .wrapping_add(3i32.wrapping_mul(dc[23]))
                                .wrapping_add(3i32.wrapping_mul(dc[24]))
                                .wrapping_add(dc[25])
                        } else {
                            7i32.wrapping_neg()
                                .wrapping_mul(dc[3])
                                .wrapping_add(50i32.wrapping_mul(dc[8]))
                                .wrapping_sub(50i32.wrapping_mul(dc[18]))
                                .wrapping_add(7i32.wrapping_mul(dc[23]))
                        });
                        workspace[8] = smooth_pred(num, q10, coef_bits[2], true) as i16;
                    }
                    // coef_bits[3] / workspace[16] (q20)
                    if coef_bits[3] != 0 && workspace[16] == 0 {
                        let num = q00.wrapping_mul(if change_dc {
                            dc[3]
                                .wrapping_add(2i32.wrapping_mul(dc[7]))
                                .wrapping_add(7i32.wrapping_mul(dc[8]))
                                .wrapping_add(2i32.wrapping_mul(dc[9]))
                                .wrapping_sub(5i32.wrapping_mul(dc[12]))
                                .wrapping_sub(14i32.wrapping_mul(dc[13]))
                                .wrapping_sub(5i32.wrapping_mul(dc[14]))
                                .wrapping_add(2i32.wrapping_mul(dc[17]))
                                .wrapping_add(7i32.wrapping_mul(dc[18]))
                                .wrapping_add(2i32.wrapping_mul(dc[19]))
                                .wrapping_add(dc[23])
                        } else {
                            dc[3]
                                .wrapping_neg()
                                .wrapping_add(13i32.wrapping_mul(dc[8]))
                                .wrapping_sub(24i32.wrapping_mul(dc[13]))
                                .wrapping_add(13i32.wrapping_mul(dc[18]))
                                .wrapping_sub(dc[23])
                        });
                        workspace[16] = smooth_pred(num, q20, coef_bits[3], true) as i16;
                    }
                    // coef_bits[4] / workspace[9] (q11)
                    if coef_bits[4] != 0 && workspace[9] == 0 {
                        let num = q00.wrapping_mul(if change_dc {
                            dc[1]
                                .wrapping_neg()
                                .wrapping_add(dc[5])
                                .wrapping_add(9i32.wrapping_mul(dc[7]))
                                .wrapping_sub(9i32.wrapping_mul(dc[9]))
                                .wrapping_sub(9i32.wrapping_mul(dc[17]))
                                .wrapping_add(9i32.wrapping_mul(dc[19]))
                                .wrapping_add(dc[21])
                                .wrapping_sub(dc[25])
                        } else {
                            dc[10]
                                .wrapping_add(dc[16])
                                .wrapping_sub(10i32.wrapping_mul(dc[17]))
                                .wrapping_add(10i32.wrapping_mul(dc[19]))
                                .wrapping_sub(dc[2])
                                .wrapping_sub(dc[20])
                                .wrapping_add(dc[22])
                                .wrapping_sub(dc[24])
                                .wrapping_add(dc[4])
                                .wrapping_sub(dc[6])
                                .wrapping_add(10i32.wrapping_mul(dc[7]))
                                .wrapping_sub(10i32.wrapping_mul(dc[9]))
                        });
                        workspace[9] = smooth_pred(num, q11, coef_bits[4], true) as i16;
                    }
                    // coef_bits[5] / workspace[2] (q02)
                    if coef_bits[5] != 0 && workspace[2] == 0 {
                        let num = q00.wrapping_mul(if change_dc {
                            2i32.wrapping_mul(dc[7])
                                .wrapping_sub(5i32.wrapping_mul(dc[8]))
                                .wrapping_add(2i32.wrapping_mul(dc[9]))
                                .wrapping_add(dc[11])
                                .wrapping_add(7i32.wrapping_mul(dc[12]))
                                .wrapping_sub(14i32.wrapping_mul(dc[13]))
                                .wrapping_add(7i32.wrapping_mul(dc[14]))
                                .wrapping_add(dc[15])
                                .wrapping_add(2i32.wrapping_mul(dc[17]))
                                .wrapping_sub(5i32.wrapping_mul(dc[18]))
                                .wrapping_add(2i32.wrapping_mul(dc[19]))
                        } else {
                            dc[11]
                                .wrapping_neg()
                                .wrapping_add(13i32.wrapping_mul(dc[12]))
                                .wrapping_sub(24i32.wrapping_mul(dc[13]))
                                .wrapping_add(13i32.wrapping_mul(dc[14]))
                                .wrapping_sub(dc[15])
                        });
                        workspace[2] = smooth_pred(num, q02, coef_bits[5], true) as i16;
                    }
                    // coef_bits[6] / workspace[3] (q03)
                    if change_dc {
                        if coef_bits[6] != 0 && workspace[3] == 0 {
                            let num = q00.wrapping_mul(
                                dc[7]
                                    .wrapping_sub(dc[9])
                                    .wrapping_add(2i32.wrapping_mul(dc[12]))
                                    .wrapping_sub(2i32.wrapping_mul(dc[14]))
                                    .wrapping_add(dc[17])
                                    .wrapping_sub(dc[19]),
                            );
                            workspace[3] = smooth_pred(num, q03, coef_bits[6], true) as i16;
                        }
                    }
                    // coef_bits[7] / workspace[10] (q12)
                    if change_dc {
                        if coef_bits[7] != 0 && workspace[10] == 0 {
                            let num = q00.wrapping_mul(
                                dc[7]
                                    .wrapping_sub(3i32.wrapping_mul(dc[8]))
                                    .wrapping_add(dc[9])
                                    .wrapping_sub(dc[17])
                                    .wrapping_add(3i32.wrapping_mul(dc[18]))
                                    .wrapping_sub(dc[19]),
                            );
                            workspace[10] = smooth_pred(num, q12, coef_bits[7], true) as i16;
                        }
                    }
                    // coef_bits[8] / workspace[17] (q21)
                    if change_dc {
                        if coef_bits[8] != 0 && workspace[17] == 0 {
                            let num = q00.wrapping_mul(
                                dc[7]
                                    .wrapping_sub(dc[9])
                                    .wrapping_sub(3i32.wrapping_mul(dc[12]))
                                    .wrapping_add(3i32.wrapping_mul(dc[14]))
                                    .wrapping_add(dc[17])
                                    .wrapping_sub(dc[19]),
                            );
                            workspace[17] = smooth_pred(num, q21, coef_bits[8], true) as i16;
                        }
                    }
                    // coef_bits[9] / workspace[24] (q30)
                    if change_dc {
                        if coef_bits[9] != 0 && workspace[24] == 0 {
                            let num = q00.wrapping_mul(
                                dc[7]
                                    .wrapping_add(2i32.wrapping_mul(dc[8]))
                                    .wrapping_add(dc[9])
                                    .wrapping_sub(dc[17])
                                    .wrapping_sub(2i32.wrapping_mul(dc[18]))
                                    .wrapping_sub(dc[19]),
                            );
                            workspace[24] = smooth_pred(num, q30, coef_bits[9], true) as i16;
                        }
                    }
                    // workspace[0] (DC estimate)
                    if change_dc {
                        let num = q00.wrapping_mul(
                            2i32.wrapping_neg()
                                .wrapping_mul(dc[1])
                                .wrapping_sub(6i32.wrapping_mul(dc[2]))
                                .wrapping_sub(8i32.wrapping_mul(dc[3]))
                                .wrapping_sub(6i32.wrapping_mul(dc[4]))
                                .wrapping_sub(2i32.wrapping_mul(dc[5]))
                                .wrapping_sub(6i32.wrapping_mul(dc[6]))
                                .wrapping_add(6i32.wrapping_mul(dc[7]))
                                .wrapping_add(42i32.wrapping_mul(dc[8]))
                                .wrapping_add(6i32.wrapping_mul(dc[9]))
                                .wrapping_sub(6i32.wrapping_mul(dc[10]))
                                .wrapping_sub(8i32.wrapping_mul(dc[11]))
                                .wrapping_add(42i32.wrapping_mul(dc[12]))
                                .wrapping_add(152i32.wrapping_mul(dc[13]))
                                .wrapping_add(42i32.wrapping_mul(dc[14]))
                                .wrapping_sub(8i32.wrapping_mul(dc[15]))
                                .wrapping_sub(6i32.wrapping_mul(dc[16]))
                                .wrapping_add(6i32.wrapping_mul(dc[17]))
                                .wrapping_add(42i32.wrapping_mul(dc[18]))
                                .wrapping_add(6i32.wrapping_mul(dc[19]))
                                .wrapping_sub(6i32.wrapping_mul(dc[20]))
                                .wrapping_sub(2i32.wrapping_mul(dc[21]))
                                .wrapping_sub(6i32.wrapping_mul(dc[22]))
                                .wrapping_sub(8i32.wrapping_mul(dc[23]))
                                .wrapping_sub(6i32.wrapping_mul(dc[24]))
                                .wrapping_sub(2i32.wrapping_mul(dc[25])),
                        );
                        workspace[0] = smooth_pred(num, q00, 0, false) as i16;
                    }
                    let res =
                        idct_for_size(dct as i32, &workspace, &self.dct_tables[ci], &range_limit)
                            .ok_or(Error::BadDctSize)?;
                    store_block(out, ci, block_row * dct, output_col, dct, &res);
                    // DC01 = DC02, DC02 = DC03, DC03 = DC04, DC04 = DC05 and likewise in each
                    // row of five; the fifth entry of a row is not shifted.
                    for row5 in 0..5 {
                        for c in 1..=4 {
                            dc[5 * row5 + c] = dc[5 * row5 + c + 1];
                        }
                    }
                    output_col += dct;
                }
            }
        }
        self.output_imcu_row += 1;
        if self.output_imcu_row < self.total_imcu_rows {
            return Ok(Some(ConsumeResult::RowCompleted));
        }
        Ok(Some(ConsumeResult::ScanCompleted))
    }
}

impl Decompress {
    /// `entropy->decode_mcu` for the current scan: progressive or sequential Huffman.
    pub(crate) fn decode_entropy_mcu(&mut self, mcu: &mut [[i16; DCTSIZE2]]) -> Result<bool> {
        if self.progressive_mode {
            self.decode_mcu_progressive(mcu)
        } else {
            self.decode_mcu(Some(mcu))
        }
    }
}

impl CompCoefs {
    /// The block at `(row, col)` (a copy, as `jcopy_block_row` would).
    pub(crate) fn block_at(&self, row: usize, col: usize) -> [i16; DCTSIZE2] {
        self.data[self.index(row, col)]
    }
}

// Port of: libjpeg-turbo src/jdapistd.c#L171-L300 (jpeg_crop_scanline) and
//          src/jdapistd.c#L360-L646 (jpeg_skip_scanlines, read_and_discard_scanlines,
//          increment_simple_rowgroup_ctr) (libjpeg_turbo@e14cbfaa, 3.1.0)
//
// Copyright (C) 1994-1996, Thomas G. Lane. libjpeg-turbo Modifications: Copyright (C) 2010,
// 2015-2016, 2019, D. R. Commander. Rust port Copyright (C) 2025 The skia-rust Authors. Licence: IJG.
//
//! Partial scanline decompression: `jpeg_crop_scanline` and `jpeg_skip_scanlines`.
//!
//! Cropping narrows the output to an MCU-aligned column range (and rebuilds the upsampler for the
//! narrower rows). Skipping moves the decoder past rows without producing them. Where the
//! upsampler keeps context rows, the skipped rows are decoded and discarded instead, so that the
//! context state machine of the main buffer controller stays consistent.

// Clippy (pedantic) allows, for this module. The arithmetic is kept as libjpeg writes it (JDIMENSION
// is unsigned, so the subtractions wrap exactly as in C, and the `long` casts stay).
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::doc_markdown,
    clippy::manual_range_contains,
    clippy::similar_names,
    clippy::too_many_lines
)]

use crate::Decompress;
use crate::error::{Error, Result};
use crate::input::jdiv_round_up;
use crate::main_ctl::CtxState;

/// `GlobalState` values that `jpeg_crop_scanline` accepts (`DSTATE_SCANNING`, `DSTATE_BUFIMAGE`).
fn crop_state_ok(state: crate::decompress::GlobalState) -> bool {
    use crate::decompress::GlobalState;
    state == GlobalState::Scanning || state == GlobalState::BufImage
}

impl Decompress {
    /// `jpeg_crop_scanline`: restricts the output to the columns `xoffset .. xoffset + width`.
    ///
    /// Must be called after `start_decompress` and before any rows are read. `xoffset` is moved
    /// left to the nearest MCU boundary and `width` grows by the same amount, so the caller must
    /// read `xoffset` and `width` back and size its buffers from them (libjpeg's documented
    /// contract).
    ///
    /// # Errors
    ///
    /// `BadState` before `start_decompress` or after rows were read, `WidthOverflow` for a range
    /// outside the output (`JERR_WIDTH_OVERFLOW`), and `BadPrecision` for non-8-bit data.
    pub fn crop_scanline(&mut self, xoffset: &mut u32, width: &mut u32) -> Result<()> {
        if self.data_precision != 8 {
            return Err(Error::BadPrecision);
        }
        if !crop_state_ok(self.global_state) || self.output_scanline != 0 {
            return Err(Error::BadState(self.global_state as i32));
        }
        // xoffset and width must fall within the output image dimensions.
        if *width == 0 || u64::from(*xoffset) + u64::from(*width) > u64::from(self.output_width) {
            return Err(Error::WidthOverflow);
        }
        // No need to do anything if the caller wants the entire width.
        if *width == self.output_width {
            return Ok(());
        }

        // xoffset must align with an iMCU column (`_min_DCT_scaled_size` times the largest
        // horizontal sampling factor), or with one block for a single-component scan.
        let align: i32 = if self.comps_in_scan == 1 && self.num_components == 1 {
            self.min_dct_scaled_size
        } else {
            self.min_dct_scaled_size * self.max_h_samp_factor
        };
        let align_u = align as u32;
        let input_xoffset = *xoffset;
        *xoffset = (input_xoffset / align_u) * align_u;

        // Only the left edge moves; the right edge stays where the caller asked.
        *width = *width + input_xoffset - *xoffset;
        self.output_width = *width;

        // The first and last iMCU columns to decompress (single-scan decompression).
        self.master.first_imcu_col = *xoffset / align_u;
        self.master.last_imcu_col =
            (jdiv_round_up(i64::from(*xoffset + self.output_width), i64::from(align)) - 1) as u32;

        let mut reinit_upsampler = false;
        for ci in 0..self.num_components as usize {
            let single = self.comps_in_scan == 1 && self.num_components == 1;
            let hsf: u32 = if single {
                1
            } else {
                self.comp_info[ci].h_samp_factor as u32
            };

            // Set downsampled_width to the new output width.
            let orig_downsampled_width = self.comp_info[ci].downsampled_width;
            let c = self.comp_info[ci];
            let new_downsampled_width = jdiv_round_up(
                i64::from(self.output_width) * i64::from(c.h_samp_factor * c.dct_h_scaled_size),
                i64::from(self.max_h_samp_factor * self.min_dct_scaled_size),
            ) as u32;
            self.comp_info[ci].downsampled_width = new_downsampled_width;
            if new_downsampled_width < 2 && orig_downsampled_width >= 2 {
                reinit_upsampler = true;
            }

            // The first and last iMCU columns of this component (multi-scan decompression).
            self.master.first_mcu_col[ci] = (*xoffset).wrapping_mul(hsf) / align_u;
            self.master.last_mcu_col[ci] = (jdiv_round_up(
                i64::from((*xoffset + self.output_width).wrapping_mul(hsf)),
                i64::from(align),
            ) - 1) as u32;
        }

        if reinit_upsampler {
            self.master.jinit_upsampler_no_alloc = true;
            let r = self.jinit_upsampler();
            self.master.jinit_upsampler_no_alloc = false;
            r?;
        }
        Ok(())
    }

    /// `jpeg_skip_scanlines`: skips `num_lines` rows of output. Returns the number of rows
    /// skipped, which is less than `num_lines` only at the bottom of the image.
    ///
    /// # Errors
    ///
    /// `BadState` when the decompressor is not scanning, and `BadPrecision` for non-8-bit data.
    pub fn skip_scanlines(&mut self, mut num_lines: u32) -> Result<u32> {
        if self.data_precision != 8 {
            return Err(Error::BadPrecision);
        }
        // Two-pass color quantization is not supported (Skia never selects it).
        if self.global_state != crate::decompress::GlobalState::Scanning {
            return Err(Error::BadState(self.global_state as i32));
        }

        // Do not skip past the bottom of the image.
        if u64::from(self.output_scanline) + u64::from(num_lines) >= u64::from(self.output_height) {
            num_lines = self.output_height - self.output_scanline;
            self.output_scanline = self.output_height;
            self.finish_input_pass();
            self.inputctl.eoi_reached = true;
            return Ok(num_lines);
        }

        if num_lines == 0 {
            return Ok(0);
        }

        let lines_per_imcu_row = (self.min_dct_scaled_size * self.max_v_samp_factor) as u32;
        let lines_left_in_imcu_row =
            (lines_per_imcu_row - (self.output_scanline % lines_per_imcu_row)) % lines_per_imcu_row;
        let mut lines_after_imcu_row = num_lines.wrapping_sub(lines_left_in_imcu_row);

        // Skip the lines remaining in the current iMCU row. When upsampling requires context
        // rows, the previous and next rows are needed to read the current one.
        if self.upsample.need_context_rows {
            // If the skipped lines would not move past the current iMCU row, read and discard.
            if num_lines < lines_left_in_imcu_row + 1
                || (lines_left_in_imcu_row <= 1
                    && self.main.buffer_full
                    && lines_after_imcu_row < lines_per_imcu_row + 1)
            {
                self.read_and_discard_scanlines(num_lines)?;
                return Ok(num_lines);
            }

            // If the next iMCU row has already been entropy-decoded, do not skip too far.
            if lines_left_in_imcu_row <= 1 && self.main.buffer_full {
                self.output_scanline += lines_left_in_imcu_row + lines_per_imcu_row;
                lines_after_imcu_row = lines_after_imcu_row.wrapping_sub(lines_per_imcu_row);
            } else {
                self.output_scanline += lines_left_in_imcu_row;
            }

            // If we have just completed the first block, adjust the buffer pointers.
            if self.main.imcu_row_ctr == 0
                || (self.main.imcu_row_ctr == 1 && lines_left_in_imcu_row > 2)
            {
                self.set_wraparound_pointers();
            }
            self.main.buffer_full = false;
            self.main.rowgroup_ctr = 0;
            self.main.context_state = CtxState::PrepareForImcu;
            self.upsample.next_row_out = self.max_v_samp_factor;
            self.upsample.rows_to_go = self.output_height - self.output_scanline;
        }
        // Skipping is much simpler when context rows are not required.
        else if num_lines < lines_left_in_imcu_row {
            self.increment_simple_rowgroup_ctr(num_lines)?;
            return Ok(num_lines);
        } else {
            self.output_scanline += lines_left_in_imcu_row;
            self.main.buffer_full = false;
            self.main.rowgroup_ctr = 0;
            self.upsample.next_row_out = self.max_v_samp_factor;
            self.upsample.rows_to_go = self.output_height - self.output_scanline;
        }

        // Calculate how many full iMCU rows we can skip.
        let lines_to_skip = if self.upsample.need_context_rows {
            (lines_after_imcu_row.wrapping_sub(1) / lines_per_imcu_row) * lines_per_imcu_row
        } else {
            (lines_after_imcu_row / lines_per_imcu_row) * lines_per_imcu_row
        };
        // The lines that remain after the full iMCU rows; they are read unless skipped.
        let lines_to_read = lines_after_imcu_row - lines_to_skip;

        // With multiple scans (progressive, non-interleaved) or buffered output, all entropy
        // decoding happens in jpeg_start_decompress, so skipping is easy.
        if self.inputctl.has_multiple_scans || self.buffered_image {
            if self.upsample.need_context_rows {
                self.output_scanline += lines_to_skip;
                self.output_imcu_row += lines_to_skip / lines_per_imcu_row;
                self.main.imcu_row_ctr += lines_to_skip / lines_per_imcu_row;
                // It is complex to move to the middle of a context block, so read the rest.
                self.read_and_discard_scanlines(lines_to_read)?;
            } else {
                self.output_scanline += lines_to_skip;
                self.output_imcu_row += lines_to_skip / lines_per_imcu_row;
                self.increment_simple_rowgroup_ctr(lines_to_read)?;
            }
            self.upsample.rows_to_go = self.output_height - self.output_scanline;
            return Ok(num_lines);
        }

        // Skip the iMCU rows that can be skipped: entropy-decode them without output.
        let mut i = 0u32;
        while i < lines_to_skip {
            for _y in 0..self.coef.mcu_rows_per_imcu_row {
                for _x in 0..self.mcus_per_row {
                    // Decoding with no output discards the coefficients (decode_mcu(NULL)).
                    if !self.insufficient_data {
                        self.master.last_good_imcu_row = self.input_imcu_row;
                    }
                    let _ = self.decode_mcu(None)?;
                }
            }
            self.input_imcu_row += 1;
            self.output_imcu_row += 1;
            if self.input_imcu_row < self.total_imcu_rows {
                self.start_imcu_row();
            } else {
                self.finish_input_pass();
            }
            i += lines_per_imcu_row;
        }
        self.output_scanline += lines_to_skip;

        if self.upsample.need_context_rows {
            // Context-based upsampling keeps track of iMCU rows.
            self.main.imcu_row_ctr += lines_to_skip / lines_per_imcu_row;
            // It is complex to move to the middle of a context block, so read the rest.
            self.read_and_discard_scanlines(lines_to_read)?;
        } else {
            self.increment_simple_rowgroup_ctr(lines_to_read)?;
        }

        // Skipping lines skips the upsampling step, so rows_to_go is set from output_scanline.
        self.upsample.rows_to_go = self.output_height - self.output_scanline;

        // Always skip the requested number of lines.
        Ok(num_lines)
    }

    /// `read_and_discard_scanlines`: decodes `num_lines` rows through the upsampler without
    /// converting their colour, so the upsampler's context state advances as if they were read.
    fn read_and_discard_scanlines(&mut self, num_lines: u32) -> Result<()> {
        self.cconvert.discard = true;
        let mut dummy: [u8; 0] = [];
        let mut result = Ok(());
        for _ in 0..num_lines {
            let mut rows: [&mut [u8]; 1] = [&mut dummy[..]];
            if let Err(e) = self.read_scanlines(&mut rows) {
                result = Err(e);
                break;
            }
        }
        self.cconvert.discard = false;
        result
    }

    /// `increment_simple_rowgroup_ctr`: skips `rows` rows of a context-free upsampler. Whole row
    /// groups are skipped by counting; a partial row group is read and discarded.
    fn increment_simple_rowgroup_ctr(&mut self, rows: u32) -> Result<()> {
        // Increment the counter to the next row group after the skipped rows.
        let max_v = self.max_v_samp_factor as u32;
        self.main.rowgroup_ctr += rows / max_v;

        // Partially skipping a row group would modify the upsampler's state, so the remaining
        // rows are read into a dummy buffer.
        let rows_left = rows % max_v;
        self.output_scanline += rows - rows_left;

        self.read_and_discard_scanlines(rows_left)
    }
}

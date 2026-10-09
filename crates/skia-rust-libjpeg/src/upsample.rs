// Port of: libjpeg-turbo src/jdsample.c#L28-L553 (libjpeg_turbo@e14cbfaa, 3.1.0)
//
// Copyright (C) 1991-1996, Thomas G. Lane. libjpeg-turbo Modifications: Copyright (C) 2010,
// 2015-2016, 2019, D. R. Commander. Rust port Copyright (C) 2025 The skia-rust Authors. Licence: IJG.
//
//! Separate upsampling (`sep_upsample`) and the per-component methods: `fullsize`, `int`,
//! `h2v1` and `h2v2` (box), and the fancy `h2v1`, `h2v2` and `h1v2` (triangle) upsamplers.
//!
//! Fancy upsampling is what Skia gets (`do_fancy_upsampling` is on by default). The merged
//! upsampler (`jdmerge.c`) is not used: `master.rs` refuses the combination that would select it.

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
use crate::tables::CompInfo;

/// The per-component upsampling method (`upsample->methods[ci]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum UpMethod {
    /// `noop_upsample`: the component is not needed.
    #[default]
    Noop,
    /// `fullsize_upsample`: the input rows are used directly.
    Fullsize,
    /// `int_upsample`: integer replication.
    Int,
    /// `h2v1_upsample`: horizontal box.
    H2v1,
    /// `h2v1_fancy_upsample`.
    H2v1Fancy,
    /// `h2v2_upsample`: box.
    H2v2,
    /// `h2v2_fancy_upsample`.
    H2v2Fancy,
    /// `h1v2_fancy_upsample`.
    H1v2Fancy,
}

/// `my_upsampler`.
#[derive(Debug, Clone, Default)]
pub(crate) struct UpsampleState {
    /// `color_buf[ci]`: the upsampled rows of each component (max_v rows).
    pub(crate) color_buf: Vec<Vec<Vec<u8>>>,
    /// `methods[ci]`.
    pub(crate) methods: Vec<UpMethod>,
    /// `next_row_out`: rows emitted from `color_buf`.
    pub(crate) next_row_out: i32,
    /// `rows_to_go`: rows remaining in the image.
    pub(crate) rows_to_go: u32,
    /// `rowgroup_height[ci]`.
    pub(crate) rowgroup_height: Vec<i32>,
    /// `h_expand[ci]`, `v_expand[ci]`.
    pub(crate) h_expand: Vec<i32>,
    /// Vertical expansion per component.
    pub(crate) v_expand: Vec<i32>,
    /// `need_context_rows`: the fancy vertical upsamplers need the rows above and below.
    pub(crate) need_context_rows: bool,
}

/// Read-only view of the input rows of one component: `rows[base + k]` is input row `k`.
pub(crate) struct InRows<'a> {
    pub(crate) rows: &'a [&'a [u8]],
    pub(crate) base: usize,
}

impl InRows<'_> {
    /// Input row `k` (may be -1 or past the group for the context rows).
    #[inline]
    fn row(&self, k: isize) -> &[u8] {
        self.rows[(self.base as isize + k) as usize]
    }
}

impl Decompress {
    /// `start_pass_upsample`.
    pub(crate) fn start_pass_upsample(&mut self) -> Result<()> {
        self.upsample.next_row_out = self.max_v_samp_factor;
        self.upsample.rows_to_go = self.output_height;
        Ok(())
    }

    /// `jinit_upsampler`: chooses the method for each component from the sampling factors.
    pub(crate) fn jinit_upsampler(&mut self) -> Result<()> {
        if self.data_precision != 8 {
            return Err(Error::BadPrecision);
        }
        if self.ccir601_sampling {
            return Err(Error::NotImplemented);
        }
        let n = self.num_components as usize;
        let do_fancy = self.do_fancy_upsampling && self.min_dct_scaled_size > 1;
        let mut methods = vec![UpMethod::Noop; n];
        let mut h_expand = vec![0i32; n];
        let mut v_expand = vec![0i32; n];
        let mut rowgroup_height = vec![0i32; n];
        let mut need_context_rows = false;
        let no_alloc = self.master.jinit_upsampler_no_alloc;
        let mut color_buf = vec![Vec::new(); n];
        for ci in 0..n {
            let c: CompInfo = self.comp_info[ci];
            let h_in_group = (c.h_samp_factor * c.dct_h_scaled_size) / self.min_dct_scaled_size;
            let v_in_group = (c.v_samp_factor * c.dct_v_scaled_size) / self.min_dct_scaled_size;
            let h_out_group = self.max_h_samp_factor;
            let v_out_group = self.max_v_samp_factor;
            rowgroup_height[ci] = v_in_group;
            let mut need_buffer = true;
            if !c.component_needed {
                methods[ci] = UpMethod::Noop;
                need_buffer = false;
            } else if h_in_group == h_out_group && v_in_group == v_out_group {
                methods[ci] = UpMethod::Fullsize;
                need_buffer = false;
            } else if h_in_group * 2 == h_out_group && v_in_group == v_out_group {
                if do_fancy && c.downsampled_width > 2 {
                    methods[ci] = UpMethod::H2v1Fancy;
                } else {
                    methods[ci] = UpMethod::H2v1;
                }
            } else if h_in_group == h_out_group && v_in_group * 2 == v_out_group && do_fancy {
                methods[ci] = UpMethod::H1v2Fancy;
                need_context_rows = true;
            } else if h_in_group * 2 == h_out_group && v_in_group * 2 == v_out_group {
                if do_fancy && c.downsampled_width > 2 {
                    methods[ci] = UpMethod::H2v2Fancy;
                    need_context_rows = true;
                } else {
                    methods[ci] = UpMethod::H2v2;
                }
            } else if h_out_group % h_in_group == 0 && v_out_group % v_in_group == 0 {
                methods[ci] = UpMethod::Int;
                h_expand[ci] = h_out_group / h_in_group;
                v_expand[ci] = v_out_group / v_in_group;
            } else {
                return Err(Error::FractSampleNotImplemented);
            }
            if need_buffer && !no_alloc {
                let width = round_up(self.output_width as usize, self.max_h_samp_factor as usize);
                color_buf[ci] = vec![vec![0u8; width]; self.max_v_samp_factor as usize];
            }
        }
        if no_alloc {
            // `jinit_upsampler_no_alloc` (jpeg_crop_scanline): the buffers, the row counters and
            // `need_context_rows` are kept; only the methods and the expansion factors change.
            // `need_context_rows` is only ever set here, as in the C branches.
            self.upsample.methods = methods;
            self.upsample.rowgroup_height = rowgroup_height;
            self.upsample.h_expand = h_expand;
            self.upsample.v_expand = v_expand;
            self.upsample.need_context_rows |= need_context_rows;
        } else {
            self.upsample = UpsampleState {
                color_buf,
                methods,
                next_row_out: self.max_v_samp_factor,
                rows_to_go: self.output_height,
                rowgroup_height,
                h_expand,
                v_expand,
                need_context_rows,
            };
        }
        Ok(())
    }

    /// `sep_upsample`: upsamples the given row group of every component, then colour-converts
    /// and emits up to `out_rows_avail - *out_row_ctr` rows into `output`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn upsample_pass(
        &mut self,
        input: &[Vec<Vec<u8>>],
        lists: &[Vec<usize>],
        bases: &[usize],
        in_row_group_ctr: &mut u32,
        _in_row_groups_avail: u32,
        output: &mut [&mut [u8]],
        out_row_ctr: &mut usize,
        out_rows_avail: usize,
    ) -> Result<()> {
        let max_v = self.max_v_samp_factor;
        if self.upsample.next_row_out >= max_v {
            for ci in 0..self.num_components as usize {
                let method = self.upsample.methods[ci];
                if method == UpMethod::Noop {
                    continue;
                }
                let c = self.comp_info[ci];
                let base = bases[ci]
                    + (*in_row_group_ctr as usize) * self.upsample.rowgroup_height[ci] as usize;
                let row_refs: Vec<&[u8]> =
                    lists[ci].iter().map(|&r| input[ci][r].as_slice()).collect();
                let inr = InRows {
                    rows: &row_refs,
                    base,
                };
                match method {
                    UpMethod::Fullsize => {
                        // The input rows themselves are the colour-convert source.
                        let rows: Vec<Vec<u8>> = (0..max_v as usize)
                            .map(|k| inr.row(k as isize).to_vec())
                            .collect();
                        self.upsample.color_buf[ci] = rows;
                    }
                    _ => {
                        let outrows = &mut self.upsample.color_buf[ci];
                        let out_w = self.output_width as usize;
                        let h_exp = self.upsample.h_expand[ci];
                        let v_exp = self.upsample.v_expand[ci];
                        match method {
                            UpMethod::Int => {
                                int_upsample(&inr, outrows, out_w, max_v, h_exp, v_exp);
                            }
                            UpMethod::H2v1 => h2v1_upsample(&inr, outrows, out_w, max_v),
                            UpMethod::H2v2 => h2v2_upsample(&inr, outrows, out_w, max_v),
                            UpMethod::H2v1Fancy => h2v1_fancy_upsample(
                                &inr,
                                outrows,
                                c.downsampled_width as usize,
                                max_v,
                            ),
                            UpMethod::H2v2Fancy => h2v2_fancy_upsample(
                                &inr,
                                outrows,
                                c.downsampled_width as usize,
                                max_v,
                            ),
                            UpMethod::H1v2Fancy => h1v2_fancy_upsample(
                                &inr,
                                outrows,
                                c.downsampled_width as usize,
                                max_v,
                            ),
                            UpMethod::Fullsize | UpMethod::Noop => {}
                        }
                    }
                }
            }
            self.upsample.next_row_out = 0;
        }

        // Color-convert and emit rows.
        let mut num_rows = (max_v - self.upsample.next_row_out) as u32;
        if num_rows > self.upsample.rows_to_go {
            num_rows = self.upsample.rows_to_go;
        }
        let avail = out_rows_avail - *out_row_ctr;
        if num_rows as usize > avail {
            num_rows = avail as u32;
        }
        let first = self.upsample.next_row_out as usize;
        if !self.cconvert.discard {
            self.color_convert(first, &mut output[*out_row_ctr..], num_rows as usize)?;
        }
        *out_row_ctr += num_rows as usize;
        self.upsample.rows_to_go -= num_rows;
        self.upsample.next_row_out += num_rows as i32;
        if self.upsample.next_row_out >= max_v {
            *in_row_group_ctr += 1;
        }
        Ok(())
    }
}

/// `jround_up(a, b)`.
fn round_up(a: usize, b: usize) -> usize {
    let a = a + b - 1;
    a - (a % b)
}

/// `int_upsample`.
fn int_upsample(
    inr: &InRows<'_>,
    out: &mut [Vec<u8>],
    out_w: usize,
    max_v: i32,
    h_expand: i32,
    v_expand: i32,
) {
    let mut inrow = 0usize;
    let mut outrow = 0usize;
    while outrow < max_v as usize {
        let inptr = inr.row(inrow as isize);
        let mut ic = 0usize;
        let mut oc = 0usize;
        // One input sample per h_expand output samples, until the output row is full.
        while oc < out_w {
            let invalue = inptr[ic];
            ic += 1;
            for _ in 0..h_expand {
                out[outrow][oc] = invalue;
                oc += 1;
            }
        }
        // Copy the row to the other output rows of this expansion group.
        if v_expand > 1 {
            for k in 1..(v_expand as usize) {
                let (a, b) = out.split_at_mut(outrow + k);
                b[0][..out_w].copy_from_slice(&a[outrow][..out_w]);
            }
        }
        inrow += 1;
        outrow += v_expand as usize;
    }
}

/// `h2v1_upsample`.
fn h2v1_upsample(inr: &InRows<'_>, out: &mut [Vec<u8>], out_w: usize, max_v: i32) {
    for inrow in 0..max_v as usize {
        let inptr = inr.row(inrow as isize);
        let outptr = &mut out[inrow];
        let mut oc = 0usize;
        let mut ic = 0usize;
        while oc < out_w {
            let invalue = inptr[ic];
            ic += 1;
            outptr[oc] = invalue;
            outptr[oc + 1] = invalue;
            oc += 2;
        }
    }
}

/// `h2v2_upsample`.
fn h2v2_upsample(inr: &InRows<'_>, out: &mut [Vec<u8>], out_w: usize, max_v: i32) {
    let mut inrow = 0usize;
    let mut outrow = 0usize;
    while outrow < max_v as usize {
        let inptr = inr.row(inrow as isize);
        {
            let outptr = &mut out[outrow];
            let mut oc = 0usize;
            let mut ic = 0usize;
            while oc < out_w {
                let invalue = inptr[ic];
                ic += 1;
                outptr[oc] = invalue;
                outptr[oc + 1] = invalue;
                oc += 2;
            }
        }
        let (a, b) = out.split_at_mut(outrow + 1);
        b[0][..out_w].copy_from_slice(&a[outrow][..out_w]);
        inrow += 1;
        outrow += 2;
    }
}

/// `h2v1_fancy_upsample`.
fn h2v1_fancy_upsample(
    inr: &InRows<'_>,
    out: &mut [Vec<u8>],
    downsampled_width: usize,
    max_v: i32,
) {
    for inrow in 0..max_v as usize {
        let inptr = inr.row(inrow as isize);
        let outptr = &mut out[inrow];
        let mut ic = 0usize;
        let mut oc = 0usize;
        // Special case for first column.
        let mut invalue = i32::from(inptr[ic]);
        ic += 1;
        outptr[oc] = invalue as u8;
        oc += 1;
        outptr[oc] = ((invalue * 3 + i32::from(inptr[ic]) + 2) >> 2) as u8;
        oc += 1;
        let mut colctr = downsampled_width as i64 - 2;
        while colctr > 0 {
            invalue = i32::from(inptr[ic]) * 3;
            ic += 1;
            outptr[oc] = ((invalue + i32::from(inptr[ic - 2]) + 1) >> 2) as u8;
            oc += 1;
            outptr[oc] = ((invalue + i32::from(inptr[ic]) + 2) >> 2) as u8;
            oc += 1;
            colctr -= 1;
        }
        // Special case for last column.
        invalue = i32::from(inptr[ic]);
        outptr[oc] = ((invalue * 3 + i32::from(inptr[ic - 1]) + 1) >> 2) as u8;
        oc += 1;
        outptr[oc] = invalue as u8;
    }
}

/// `h2v2_fancy_upsample`.
fn h2v2_fancy_upsample(
    inr: &InRows<'_>,
    out: &mut [Vec<u8>],
    downsampled_width: usize,
    max_v: i32,
) {
    let mut inrow = 0isize;
    let mut outrow = 0usize;
    while outrow < max_v as usize {
        for v in 0..2 {
            let inptr0 = inr.row(inrow);
            let inptr1 = if v == 0 {
                inr.row(inrow - 1)
            } else {
                inr.row(inrow + 1)
            };
            let outptr = &mut out[outrow];
            outrow += 1;
            let mut ic = 0usize;
            let mut oc = 0usize;
            // Special case for first column.
            let mut thiscolsum = i32::from(inptr0[ic]) * 3 + i32::from(inptr1[ic]);
            ic += 1;
            let mut nextcolsum = i32::from(inptr0[ic]) * 3 + i32::from(inptr1[ic]);
            ic += 1;
            outptr[oc] = ((thiscolsum * 4 + 8) >> 4) as u8;
            oc += 1;
            outptr[oc] = ((thiscolsum * 3 + nextcolsum + 7) >> 4) as u8;
            oc += 1;
            let mut lastcolsum = thiscolsum;
            thiscolsum = nextcolsum;
            let mut colctr = downsampled_width as i64 - 2;
            while colctr > 0 {
                nextcolsum = i32::from(inptr0[ic]) * 3 + i32::from(inptr1[ic]);
                ic += 1;
                outptr[oc] = ((thiscolsum * 3 + lastcolsum + 8) >> 4) as u8;
                oc += 1;
                outptr[oc] = ((thiscolsum * 3 + nextcolsum + 7) >> 4) as u8;
                oc += 1;
                lastcolsum = thiscolsum;
                thiscolsum = nextcolsum;
                colctr -= 1;
            }
            // Special case for last column.
            outptr[oc] = ((thiscolsum * 3 + lastcolsum + 8) >> 4) as u8;
            oc += 1;
            outptr[oc] = ((thiscolsum * 4 + 7) >> 4) as u8;
        }
        inrow += 1;
    }
}

/// `h1v2_fancy_upsample`.
fn h1v2_fancy_upsample(
    inr: &InRows<'_>,
    out: &mut [Vec<u8>],
    downsampled_width: usize,
    max_v: i32,
) {
    let mut inrow = 0isize;
    let mut outrow = 0usize;
    while outrow < max_v as usize {
        for v in 0..2 {
            let inptr0 = inr.row(inrow);
            let (inptr1, bias) = if v == 0 {
                (inr.row(inrow - 1), 1)
            } else {
                (inr.row(inrow + 1), 2)
            };
            let outptr = &mut out[outrow];
            outrow += 1;
            for col in 0..downsampled_width {
                let thiscolsum = i32::from(inptr0[col]) * 3 + i32::from(inptr1[col]);
                outptr[col] = ((thiscolsum + bias) >> 2) as u8;
            }
        }
        inrow += 1;
    }
}

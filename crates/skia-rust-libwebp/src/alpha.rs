// Copyright 2012 Google Inc. All Rights Reserved.
//
// Use of this source code is governed by a BSD-style license that can be
// found in the COPYING file. Port by The skia-rust Authors.

//! Port of libwebp `src/dec/alpha_dec.c` (the `ALPH` chunk decoder) and the decoder half of
//! `src/dsp/filters.c` (the unfilters `WebPUnfilters`).
//!
//! The alpha plane is stored either raw (method 0) or as a lossless VP8L image without the
//! header (method 1). The optional spatial filter is undone row by row. Dithering
//! (`WebPDequantizeLevels`) is not ported: Skia's `WebPDecoderConfig` leaves
//! `dithering_strength` at 0, so it never runs.

// Module-level clippy allows. Each one mirrors the C source of this module.
// clippy::cast_possible_truncation: C integer conversions (int, uint8_t, uint16_t, uint32_t, size_t) are written as `as` casts of the same width and sign as in the C source.
// clippy::cast_sign_loss: C integer conversions (int, uint8_t, uint16_t, uint32_t, size_t) are written as `as` casts of the same width and sign as in the C source.
// clippy::similar_names: local names mirror the C identifiers, which differ by one letter or a suffix.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::similar_names
)]

use crate::vp8l::{self, Vp8lDecoder};

/// Port of `ALPHA_HEADER_LEN`.
pub const ALPHA_HEADER_LEN: usize = 1;

/// Port of `WEBP_FILTER_TYPE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[doc(alias = "WEBP_FILTER_TYPE")]
pub enum Filter {
    #[default]
    None = 0,
    Horizontal = 1,
    Vertical = 2,
    Gradient = 3,
}

impl Filter {
    fn from_bits(bits: u8) -> Self {
        match bits {
            0 => Self::None,
            1 => Self::Horizontal,
            2 => Self::Vertical,
            _ => Self::Gradient,
        }
    }
}

/// Port of `GradientPredictor_C`.
#[inline]
fn gradient_predictor(a: u8, b: u8, c: u8) -> u8 {
    let g = i32::from(a) + i32::from(b) - i32::from(c);
    if (g & !0xff) == 0 {
        g as u8
    } else if g < 0 {
        0
    } else {
        255
    }
}

/// Port of `WebPUnfilters[filter]`: reconstructs one row `out` from the residuals `input` and
/// the previous reconstructed row `prev` (`None` for the first row).
pub fn unfilter(filter: Filter, prev: Option<&[u8]>, input: &[u8], out: &mut [u8], width: usize) {
    match filter {
        Filter::None => out[..width].copy_from_slice(&input[..width]),
        Filter::Horizontal => horizontal_unfilter(prev, input, out, width),
        Filter::Vertical => match prev {
            None => horizontal_unfilter(None, input, out, width),
            Some(p) => {
                for i in 0..width {
                    out[i] = p[i].wrapping_add(input[i]);
                }
            }
        },
        Filter::Gradient => match prev {
            None => horizontal_unfilter(None, input, out, width),
            Some(p) => {
                let mut top = p[0];
                let mut top_left = top;
                let mut left = top;
                for i in 0..width {
                    top = p[i]; // need to read this first, in case prev==out
                    left = input[i].wrapping_add(gradient_predictor(left, top, top_left));
                    top_left = top;
                    out[i] = left;
                }
            }
        },
    }
}

/// Port of `HorizontalUnfilter_C`.
fn horizontal_unfilter(prev: Option<&[u8]>, input: &[u8], out: &mut [u8], width: usize) {
    let mut pred = prev.map_or(0, |p| p[0]);
    for i in 0..width {
        out[i] = pred.wrapping_add(input[i]);
        pred = out[i];
    }
}

/// The output plane of an alpha decoder, with the state that `AlphaApplyFilter` keeps.
#[derive(Debug, Default)]
#[doc(alias = "ALPHDecoder")]
pub(crate) struct AlphaOutput {
    /// `alph_dec->output_`: `width * height` bytes.
    pub output: Vec<u8>,
    /// The final image width (`io->width`).
    pub width: i32,
    pub crop_top: i32,
    pub filter: Filter,
    /// `prev_line_`: offset in `output` of the last row that was unfiltered.
    pub prev_line: Option<usize>,
}

/// `AlphaApplyFilter`: unfilters rows `first_row..last_row` of `alph.output`, which start at byte
/// offset `out_off` for `first_row`.
pub(crate) fn apply_filter(alph: &mut AlphaOutput, first_row: i32, last_row: i32, out_off: usize) {
    if alph.filter == Filter::None {
        return;
    }
    let width = alph.width as usize;
    let mut prev = alph.prev_line;
    let mut out = out_off;
    for _y in first_row..last_row {
        let input = alph.output[out..out + width].to_vec();
        let prev_row = prev.map(|p| alph.output[p..p + width].to_vec());
        unfilter(
            alph.filter,
            prev_row.as_deref(),
            &input,
            &mut alph.output[out..out + width],
            width,
        );
        prev = Some(out);
        out += width;
    }
    alph.prev_line = prev;
}

/// Port of `ALPHDecoder`: one `ALPH` chunk being decoded.
#[derive(Debug)]
#[doc(alias = "ALPHDecoder")]
pub(crate) struct AlphaDecoder {
    method: u8,
    /// The alpha bytes after the one-byte `ALPH` header.
    data: Vec<u8>,
    pub output: AlphaOutput,
    /// `ALPHDecoder::vp8l_dec_` and `use_8b_decode_`, for method 1.
    vp8l: Option<(Vp8lDecoder, bool)>,
    /// `dec->alpha_prev_line_` for method 0.
    prev_line_raw: Option<usize>,
    /// Rows already decoded for method 0.
    rows_done: i32,
}

/// Port of `ALPHInit`. `src_width`/`src_height` and the crop window come from the VP8 frame.
#[doc(alias = "ALPHInit")]
pub(crate) fn alpha_init(
    data: &[u8],
    src_width: i32,
    src_height: i32,
    crop: (i32, i32, i32, i32),
) -> Option<AlphaDecoder> {
    // data: the full ALPH payload (header + alpha data)
    if data.len() <= ALPHA_HEADER_LEN {
        return None;
    }
    let method = data[0] & 0x03;
    let filter = Filter::from_bits((data[0] >> 2) & 0x03);
    let pre_processing = (data[0] >> 4) & 0x03;
    let rsrv = (data[0] >> 6) & 0x03;
    if method > 1 || pre_processing > 1 || rsrv != 0 {
        return None;
    }
    let alpha_data = data[ALPHA_HEADER_LEN..].to_vec();
    let (_crop_left, _crop_right, crop_top, _crop_bottom) = crop;
    let width = src_width;
    let height = src_height;
    let mut output = AlphaOutput {
        output: vec![0u8; (width as usize) * (height as usize)],
        width,
        crop_top,
        filter,
        prev_line: None,
    };
    let mut dec = AlphaDecoder {
        method,
        data: alpha_data,
        output: AlphaOutput::default(),
        vp8l: None,
        prev_line_raw: None,
        rows_done: 0,
    };
    if method == 0 {
        // ALPHA_NO_COMPRESSION
        let decoded_size = (width as usize) * (height as usize);
        if dec.data.len() < decoded_size {
            return None;
        }
    } else {
        // ALPHA_LOSSLESS_COMPRESSION
        let (vp8l_dec, use_8b) = vp8l::alpha_header(&dec.data, width, height)?;
        dec.vp8l = Some((vp8l_dec, use_8b));
    }
    output.filter = filter;
    dec.output = output;
    Some(dec)
}

/// Port of `ALPHDecode`: decodes rows `row..row + num_rows`. Returns `false` on a bitstream error.
#[doc(alias = "ALPHDecode")]
pub(crate) fn alpha_decode(dec: &mut AlphaDecoder, row: i32, num_rows: i32) -> bool {
    let width = dec.output.width as usize;
    if dec.method == 0 {
        // ALPHA_NO_COMPRESSION
        let filter = dec.output.filter;
        let mut prev = dec.prev_line_raw;
        for y in 0..num_rows as usize {
            let dst = (row as usize + y) * width;
            let deltas_start = (row as usize + y) * width;
            let input = dec.data[deltas_start..deltas_start + width].to_vec();
            let prev_row = prev.map(|p| dec.output.output[p..p + width].to_vec());
            unfilter(
                filter,
                prev_row.as_deref(),
                &input,
                &mut dec.output.output[dst..dst + width],
                width,
            );
            prev = Some(dst);
        }
        dec.prev_line_raw = prev;
        dec.rows_done = row + num_rows;
        return true;
    }
    // ALPHA_LOSSLESS_COMPRESSION
    let Some((vp8l_dec, use_8b)) = dec.vp8l.as_mut() else {
        return false;
    };
    let mut alph = std::mem::take(&mut dec.output);
    let ok = vp8l::alpha_image_stream(vp8l_dec, &dec.data, *use_8b, &mut alph, row + num_rows);
    dec.output = alph;
    if ok {
        dec.rows_done = row + num_rows;
    }
    ok
}

impl AlphaDecoder {
    /// The decoded alpha plane (`width * height` bytes).
    #[must_use]
    pub fn plane(&self) -> &[u8] {
        &self.output.output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_unfilter_accumulates() {
        let input = [1u8, 1, 1];
        let mut out = [0u8; 3];
        unfilter(Filter::Horizontal, None, &input, &mut out, 3);
        assert_eq!(out, [1, 2, 3]);
    }

    #[test]
    fn uncompressed_alpha_round_trips() {
        let mut data = vec![0u8]; // method 0, filter none
        data.extend_from_slice(&[10, 20, 30, 40]);
        let mut dec = alpha_init(&data, 2, 2, (0, 2, 0, 2)).expect("valid header");
        assert!(alpha_decode(&mut dec, 0, 2));
        assert_eq!(dec.plane(), &[10, 20, 30, 40]);
    }
}

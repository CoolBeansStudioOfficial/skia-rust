// Port of: libjpeg-turbo src/jcsample.c#L1-L556 (libjpeg_turbo@e14cbfaa, 3.1.0 source):
// `expand_right_edge`, `fullsize_downsample`, `h2v1_downsample`, `h2v2_downsample`,
// `int_downsample`, the input-smoothing variants `h2v2_smooth_downsample` and
// `fullsize_smooth_downsample`, and `_jinit_downsampler`'s choice between them. The bottom padding
// of `jcprepct.c` (`expand_bottom_edge`, on the input rows and on the downsampled rows) is applied
// here as index clamping and as the copy of the last downsampled row.
//
// Copyright (C) 1991-1996, Thomas G. Lane. Modified 2009-2011 by D. R. Commander.
// Copyright (C) 2025 The skia-rust Authors.
//
// Each output sample is computed from the input samples at the same indices as in the C code.
// An index past the image edge reads the nearest real sample: that is what `expand_right_edge`
// (columns) and `expand_bottom_edge` / `set_bottom_pointers` (rows) write into the C buffers.

use super::Compress;

/// One colour-converted component plane: `width` samples per row, `height` real rows.
struct Plane<'a> {
    data: &'a [u8],
    width: i64,
    height: i64,
}

impl Plane<'_> {
    /// The sample at `(row, col)`, with out-of-image indices replaced by the nearest real sample.
    fn px(&self, row: i64, col: i64) -> i64 {
        let r = row.clamp(0, self.height - 1);
        let c = col.clamp(0, self.width - 1);
        i64::from(self.data[(r * self.width + c) as usize])
    }
}

/// `(JSAMPLE)` of a non-negative `JLONG`: keeps the low 8 bits, as the C cast does.
fn jsample(v: i64) -> u8 {
    v as u8
}

impl Compress {
    /// `_downsample` for every component: the downsampled planes, each with `width_in_blocks * 8`
    /// samples per row and `height_in_blocks * 8` rows, which is what the forward DCT reads.
    pub(super) fn downsample_all(&self) -> Vec<Vec<u8>> {
        (0..self.num_components)
            .map(|ci| self.downsample_component(ci))
            .collect()
    }

    /// `_jinit_downsampler`'s method choice for one component, applied to the whole plane.
    fn downsample_component(&self, ci: usize) -> Vec<u8> {
        let compptr = self.comp_info[ci];
        let maxh = i64::from(self.max_h_samp_factor);
        let maxv = i64::from(self.max_v_samp_factor);
        let h = i64::from(compptr.h_samp_factor);
        let v = i64::from(compptr.v_samp_factor);
        let out_w = compptr.width_in_blocks as usize * 8;
        let out_h = compptr.height_in_blocks as usize * 8;
        let width = i64::from(self.image_width);
        let height = i64::from(self.image_height);
        let plane = Plane {
            data: &self.planes[ci],
            width,
            height,
        };
        let smooth = self.smoothing_factor != 0;
        // Input row groups: ceil(height / maxv), each making v output rows. Without smoothing,
        // libjpeg's pre_process_data replicates the last computed output row past them. With
        // smoothing (any smoothing method makes the prep controller use context rows), it computes
        // the groups up to the iMCU boundary from the padded input, so every row is computed.
        let groups = (height + maxv - 1) / maxv;
        let computed_rows = if smooth {
            out_h
        } else {
            ((groups * v) as usize).min(out_h)
        };
        let mut out = vec![0u8; out_w * out_h];
        for o in 0..computed_rows {
            let dst = &mut out[o * out_w..(o + 1) * out_w];
            let o = o as i64;
            if h == maxh && v == maxv {
                if smooth {
                    fullsize_smooth_row(&plane, o, out_w, self.smoothing_factor, dst);
                } else {
                    // fullsize_downsample: a copy, with the right edge replicated.
                    for (c, d) in dst.iter_mut().enumerate() {
                        *d = plane.px(o, c as i64) as u8;
                    }
                }
            } else if h * 2 == maxh && v == maxv {
                h2v1_row(&plane, o, out_w, dst);
            } else if h * 2 == maxh && v * 2 == maxv {
                if smooth {
                    h2v2_smooth_row(&plane, o, out_w, self.smoothing_factor, dst);
                } else {
                    h2v2_row(&plane, o, out_w, dst);
                }
            } else {
                // int_downsample: a box average over h_expand x v_expand input samples.
                let h_expand = maxh / h;
                let v_expand = maxv / v;
                let numpix = h_expand * v_expand;
                let numpix2 = numpix / 2;
                let group = o / v;
                let k = o % v;
                let inrow = group * maxv + k * v_expand;
                for (outcol, d) in dst.iter_mut().enumerate() {
                    let outcol_h = outcol as i64 * h_expand;
                    let mut outvalue = 0i64;
                    for vv in 0..v_expand {
                        for hh in 0..h_expand {
                            outvalue += plane.px(inrow + vv, outcol_h + hh);
                        }
                    }
                    *d = jsample((outvalue + numpix2) / numpix);
                }
            }
        }
        // jcprepct.c's expand_bottom_edge on the downsampled rows (non-smoothing path).
        if computed_rows < out_h && computed_rows > 0 {
            let last = (computed_rows - 1) * out_w;
            for o in computed_rows..out_h {
                let src: Vec<u8> = out[last..last + out_w].to_vec();
                out[o * out_w..(o + 1) * out_w].copy_from_slice(&src);
            }
        }
        out
    }
}

/// `h2v1_downsample` for one output row `o` (the input row `o`).
fn h2v1_row(plane: &Plane<'_>, o: i64, out_w: usize, dst: &mut [u8]) {
    let mut bias = 0i64;
    for (outcol, d) in dst.iter_mut().enumerate().take(out_w) {
        let c = outcol as i64;
        let v = plane.px(o, 2 * c) + plane.px(o, 2 * c + 1) + bias;
        *d = jsample(v >> 1);
        bias ^= 1;
    }
}

/// `h2v2_downsample` for one output row `o` (the input rows `2o` and `2o + 1`).
fn h2v2_row(plane: &Plane<'_>, o: i64, out_w: usize, dst: &mut [u8]) {
    let mut bias = 1i64;
    for (outcol, d) in dst.iter_mut().enumerate().take(out_w) {
        let c = outcol as i64;
        let inrow0 = 2 * o;
        let inrow1 = 2 * o + 1;
        let v = plane.px(inrow0, 2 * c)
            + plane.px(inrow0, 2 * c + 1)
            + plane.px(inrow1, 2 * c)
            + plane.px(inrow1, 2 * c + 1)
            + bias;
        *d = jsample(v >> 2);
        bias ^= 3;
    }
}

/// `h2v2_smooth_downsample` for one output row `o`, with the smoothing factor `sf`.
fn h2v2_smooth_row(plane: &Plane<'_>, o: i64, out_w: usize, sf: i32, dst: &mut [u8]) {
    let sf = i64::from(sf);
    let memberscale = 16384 - sf * 80;
    let neighscale = sf * 16;
    let inrow = 2 * o;
    let (r0, r1, ra, rb) = (inrow, inrow + 1, inrow - 1, inrow + 2);
    let p = |row: i64, col: i64| plane.px(row, col);
    let output_cols = out_w as i64;
    // First column.
    {
        let mut membersum = p(r0, 0) + p(r0, 1) + p(r1, 0) + p(r1, 1);
        let mut neighsum =
            p(ra, 0) + p(ra, 1) + p(rb, 0) + p(rb, 1) + p(r0, 0) + p(r0, 2) + p(r1, 0) + p(r1, 2);
        neighsum += neighsum;
        neighsum += p(ra, 0) + p(ra, 2) + p(rb, 0) + p(rb, 2);
        membersum = membersum * memberscale + neighsum * neighscale;
        dst[0] = jsample((membersum + 32768) >> 16);
    }
    // Middle columns.
    for outcol in 1..output_cols - 1 {
        let c = 2 * outcol;
        let mut membersum = p(r0, c) + p(r0, c + 1) + p(r1, c) + p(r1, c + 1);
        let mut neighsum = p(ra, c)
            + p(ra, c + 1)
            + p(rb, c)
            + p(rb, c + 1)
            + p(r0, c - 1)
            + p(r0, c + 2)
            + p(r1, c - 1)
            + p(r1, c + 2);
        neighsum += neighsum;
        neighsum += p(ra, c - 1) + p(ra, c + 2) + p(rb, c - 1) + p(rb, c + 2);
        membersum = membersum * memberscale + neighsum * neighscale;
        dst[outcol as usize] = jsample((membersum + 32768) >> 16);
    }
    // Last column.
    {
        let c = 2 * (output_cols - 1);
        let mut membersum = p(r0, c) + p(r0, c + 1) + p(r1, c) + p(r1, c + 1);
        let mut neighsum = p(ra, c)
            + p(ra, c + 1)
            + p(rb, c)
            + p(rb, c + 1)
            + p(r0, c - 1)
            + p(r0, c + 1)
            + p(r1, c - 1)
            + p(r1, c + 1);
        neighsum += neighsum;
        neighsum += p(ra, c - 1) + p(ra, c + 1) + p(rb, c - 1) + p(rb, c + 1);
        membersum = membersum * memberscale + neighsum * neighscale;
        dst[(output_cols - 1) as usize] = jsample((membersum + 32768) >> 16);
    }
}

/// `fullsize_smooth_downsample` for one output row `o` (the input row `o`, with its neighbours).
fn fullsize_smooth_row(plane: &Plane<'_>, o: i64, out_w: usize, sf: i32, dst: &mut [u8]) {
    let sf = i64::from(sf);
    let memberscale = 65536 - sf * 512;
    let neighscale = sf * 64;
    let output_cols = out_w as i64;
    let p = |row: i64, col: i64| plane.px(row, col);
    let (above, inrow, below) = (o - 1, o, o + 1);
    // First column.
    let mut colsum = p(above, 0) + p(below, 0) + p(inrow, 0);
    let mut membersum = p(inrow, 0);
    let mut nextcolsum = p(above, 1) + p(below, 1) + p(inrow, 1);
    let mut neighsum = colsum + (colsum - membersum) + nextcolsum;
    membersum = membersum * memberscale + neighsum * neighscale;
    dst[0] = jsample((membersum + 32768) >> 16);
    let mut lastcolsum = colsum;
    colsum = nextcolsum;
    // Middle columns.
    for outcol in 1..output_cols - 1 {
        membersum = p(inrow, outcol);
        nextcolsum = p(above, outcol + 1) + p(below, outcol + 1) + p(inrow, outcol + 1);
        neighsum = lastcolsum + (colsum - membersum) + nextcolsum;
        membersum = membersum * memberscale + neighsum * neighscale;
        dst[outcol as usize] = jsample((membersum + 32768) >> 16);
        lastcolsum = colsum;
        colsum = nextcolsum;
    }
    // Last column.
    let last = output_cols - 1;
    membersum = p(inrow, last);
    neighsum = lastcolsum + (colsum - membersum) + colsum;
    membersum = membersum * memberscale + neighsum * neighscale;
    dst[last as usize] = jsample((membersum + 32768) >> 16);
}

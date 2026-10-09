// Port of: libjpeg-turbo src/jccoefct.c#L1-L454 (libjpeg_turbo@e14cbfaa, 3.1.0 source), the
// full-image path that Skia's optimized encoder takes (`compress_first_pass` and
// `compress_output`), and the MCU order of `jcmaster.c`'s `per_scan_setup`. The virtual block
// arrays (`request_virt_barray`, `access_virt_barray`) are whole-image `Vec`s here.
//
// Copyright (C) 1994-1997, Thomas G. Lane. Modified 2003-2009 by Guido Vollbeding.
// Copyright (C) 2009-2011, 2015, 2025 D. R. Commander.
// Copyright (C) 2025 The skia-rust Authors.

// The C loop shapes over several parallel arrays are kept (`for i in 0..n`), as in libjpeg.
#![allow(clippy::needless_range_loop)]

use super::fdct::{Divisor, forward_dct_block, make_divisors};
use super::{Block, Compress, jround_up};
use crate::error::{Error, Result};
use crate::tables::DCTSIZE;

/// One component's whole-image coefficient array: `rows` block rows of `cols` blocks each, where
/// `rows = total_iMCU_rows * v_samp_factor` and `cols = jround_up(width_in_blocks, h_samp_factor)`
/// (`request_virt_barray` in `jccoefct.c`).
#[derive(Debug, Clone)]
pub(super) struct CoefBuffer {
    /// Blocks per row of the array.
    pub(super) cols: usize,
    /// The blocks, row-major: `blocks[row * cols + col]`.
    pub(super) blocks: Vec<Block>,
}

impl CoefBuffer {
    /// The block at `(row, col)`.
    pub(super) fn at(&self, row: usize, col: usize) -> &Block {
        &self.blocks[row * self.cols + col]
    }
}

impl Compress {
    /// `compress_first_pass` for every iMCU row and component: forward DCT and quantization of
    /// the `width_in_blocks x height_in_blocks` real blocks, then the dummy blocks that complete
    /// the MCUs (DC copied from the last real block of the row) and the dummy block rows
    /// (DC copied from the block row above, per MCU).
    pub(super) fn forward_dct_all(&self, planes: &[Vec<u8>]) -> Result<Vec<CoefBuffer>> {
        let mut out = Vec::with_capacity(self.num_components);
        for ci in 0..self.num_components {
            let compptr = self.comp_info[ci];
            let qtbl = self
                .quant_tbl_ptrs
                .get(compptr.quant_tbl_no as usize)
                .and_then(Option::as_ref)
                .ok_or(Error::NoQuantTable(compptr.quant_tbl_no))?;
            let divisors: [Divisor; 64] = make_divisors(qtbl);
            let h = compptr.h_samp_factor as usize;
            let v = compptr.v_samp_factor as usize;
            let wblk = compptr.width_in_blocks as usize;
            let hblk = compptr.height_in_blocks as usize;
            let cols = jround_up(wblk as i64, h as i64) as usize;
            let rows = self.total_imcu_rows as usize * v;
            let mut buf = CoefBuffer {
                cols,
                blocks: vec![[0i16; 64]; rows * cols],
            };
            let plane = &planes[ci];
            let stride = wblk * DCTSIZE;
            let last_imcu = self.total_imcu_rows as usize - 1;
            // jccoefct.c: `ndummy` completes the last MCU column of each real block row.
            let mut ndummy = wblk % h;
            if ndummy > 0 {
                ndummy = h - ndummy;
            }
            for imcu in 0..self.total_imcu_rows as usize {
                let block_rows = if imcu < last_imcu {
                    v
                } else {
                    let t = hblk % v;
                    if t == 0 { v } else { t }
                };
                for block_row in 0..block_rows {
                    let grow = imcu * v + block_row;
                    for bc in 0..wblk {
                        buf.blocks[grow * cols + bc] = forward_dct_block(
                            plane,
                            stride,
                            grow * DCTSIZE,
                            bc * DCTSIZE,
                            &divisors,
                        );
                    }
                    if ndummy > 0 {
                        let lastdc = buf.blocks[grow * cols + wblk - 1][0];
                        for bi in 0..ndummy {
                            buf.blocks[grow * cols + wblk + bi] = [0; 64];
                            buf.blocks[grow * cols + wblk + bi][0] = lastdc;
                        }
                    }
                }
                if imcu == last_imcu {
                    // The dummy block rows below the image: zero, with the DC of the block above.
                    let blocks_across = wblk + ndummy;
                    let mcus_across = blocks_across / h;
                    for block_row in block_rows..v {
                        let grow = imcu * v + block_row;
                        let lrow = grow - 1;
                        for m in 0..mcus_across {
                            let lastdc = buf.blocks[lrow * cols + m * h + h - 1][0];
                            for bi in 0..h {
                                buf.blocks[grow * cols + m * h + bi] = [0; 64];
                                buf.blocks[grow * cols + m * h + bi][0] = lastdc;
                            }
                        }
                    }
                }
            }
            out.push(buf);
        }
        Ok(out)
    }

    /// The MCUs of the one scan, in coding order, as `(component, block)` lists. An interleaved
    /// scan has `MCUs_per_row` MCUs per iMCU row, each made of the component blocks in
    /// `MCU_membership` order. A one-component scan has one block per MCU.
    pub(super) fn for_each_mcu<F>(&self, bufs: &[CoefBuffer], mut f: F) -> Result<()>
    where
        F: FnMut(&[(usize, Block)]) -> Result<()>,
    {
        let mut mcu: Vec<(usize, Block)> = Vec::with_capacity(10);
        if self.num_components == 1 {
            // per_scan_setup, comps_in_scan == 1: MCU_width = MCU_height = 1.
            let compptr = self.comp_info[0];
            let wblk = compptr.width_in_blocks as usize;
            let hblk = compptr.height_in_blocks as usize;
            let buf = &bufs[0];
            for row in 0..hblk {
                for col in 0..wblk {
                    mcu.clear();
                    mcu.push((0, *buf.at(row, col)));
                    f(&mcu)?;
                }
            }
        } else {
            let mcus_per_row = self.mcus_per_row as usize;
            for imcu in 0..self.total_imcu_rows as usize {
                for mcu_col in 0..mcus_per_row {
                    mcu.clear();
                    for ci in 0..self.num_components {
                        let compptr = self.comp_info[ci];
                        let h = compptr.h_samp_factor as usize;
                        let v = compptr.v_samp_factor as usize;
                        for yindex in 0..v {
                            for xindex in 0..h {
                                let row = imcu * v + yindex;
                                let col = mcu_col * h + xindex;
                                mcu.push((ci, *bufs[ci].at(row, col)));
                            }
                        }
                    }
                    f(&mcu)?;
                }
            }
        }
        Ok(())
    }
}

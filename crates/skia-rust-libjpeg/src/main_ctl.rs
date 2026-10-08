// Port of: libjpeg-turbo src/jdmainct.c#L134-L482 (libjpeg_turbo@e14cbfaa, 3.1.0)
//
// Copyright (C) 1994-1996, Thomas G. Lane. libjpeg-turbo Modifications: Copyright (C) 2010,
// 2016, D. R. Commander. Rust port Copyright (C) 2025 The skia-rust Authors. Licence: IJG.
//
//! The main buffer controller: holds one iMCU row of samples (plus the context rows that fancy
//! upsampling needs) and hands row groups to the upsampler.
//!
//! libjpeg builds its context with "funny pointers": lists of row pointers that alias the same
//! row storage in different orders. Here a list is a `Vec<usize>` of row indices into the
//! component's storage, and `xbuffer[w][ci]` keeps `rgroup` extra entries in front so that the
//! negative C indices stay valid (the C pointer `xbuf` is `list[rgroup..]`).

use crate::Decompress;
use crate::error::{Error, Result};
use crate::marker::ConsumeResult;
use crate::coef::OutPlanes;

/// `CTX_PREPARE_FOR_IMCU`, `CTX_PROCESS_IMCU`, `CTX_POSTPONED_ROW`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum CtxState {
    #[default]
    PrepareForImcu,
    ProcessImcu,
    PostponedRow,
}

/// `my_main_controller`.
#[derive(Debug, Clone, Default)]
pub(crate) struct MainState {
    /// `buffer[ci]`: the sample rows of one component's iMCU row group (plus context).
    pub(crate) buffer: Vec<Vec<Vec<u8>>>,
    /// `buffer_full`: have we got an iMCU row from the decoder?
    pub(crate) buffer_full: bool,
    /// `rowgroup_ctr`: row groups output to the upsampler.
    pub(crate) rowgroup_ctr: u32,
    /// `xbuffer[2][ci]`: the funny pointer lists (with `rgroup` leading entries).
    pub(crate) xbuffer: [Vec<Vec<usize>>; 2],
    /// `rgroup` per component, the offset of index 0 in each `xbuffer` list.
    pub(crate) xbase: Vec<usize>,
    /// `whichptr`.
    pub(crate) whichptr: usize,
    /// `context_state`.
    pub(crate) context_state: CtxState,
    /// `rowgroups_avail`.
    pub(crate) rowgroups_avail: u32,
    /// `iMCU_row_ctr`.
    pub(crate) imcu_row_ctr: u32,
    /// Whether the upsampler needs context rows (`need_context_rows`).
    pub(crate) context: bool,
}

impl Decompress {
    /// `alloc_funny_pointers` (sizes only; the lists are filled by `make_funny_pointers`).
    fn alloc_funny_pointers(&mut self) {
        let m = self.min_dct_scaled_size as usize;
        let n = self.num_components as usize;
        self.main.xbase = vec![0; n];
        for w in 0..2 {
            self.main.xbuffer[w] = vec![Vec::new(); n];
        }
        for ci in 0..n {
            let c = self.comp_info[ci];
            let rgroup = ((c.v_samp_factor * c.dct_h_scaled_size) / self.min_dct_scaled_size) as usize;
            self.main.xbase[ci] = rgroup;
            for w in 0..2 {
                self.main.xbuffer[w][ci] = vec![0; rgroup * (m + 4)];
            }
        }
    }

    /// `make_funny_pointers`: the initial aliasing of the context lists.
    fn make_funny_pointers(&mut self) {
        let m = self.min_dct_scaled_size as usize;
        for ci in 0..self.num_components as usize {
            let c = self.comp_info[ci];
            let rgroup = ((c.v_samp_factor * c.dct_h_scaled_size) / self.min_dct_scaled_size) as usize;
            let base = self.main.xbase[ci];
            // xbuf0[i] = xbuf1[i] = buf[i] for i < rgroup * (M + 2)
            for i in 0..rgroup * (m + 2) {
                self.main.xbuffer[0][ci][base + i] = i;
                self.main.xbuffer[1][ci][base + i] = i;
            }
            // Middle section: rows above and below the context rows.
            for i in 0..rgroup * 2 {
                let a = rgroup * (m - 2) + i;
                let b = rgroup * m + i;
                self.main.xbuffer[1][ci][base + a] = b;
                self.main.xbuffer[1][ci][base + b] = rgroup * (m - 2) + i;
            }
            // Top context: duplicate the first row.
            for i in 0..rgroup {
                let first = self.main.xbuffer[0][ci][base];
                self.main.xbuffer[0][ci][base + i - rgroup] = first;
            }
        }
    }

    /// `set_bottom_pointers`: manufactures the rows below the last real row.
    fn set_bottom_pointers(&mut self) {
        let w = self.main.whichptr;
        for ci in 0..self.num_components as usize {
            let c = self.comp_info[ci];
            let imcu_height = (c.v_samp_factor * c.dct_h_scaled_size) as u32;
            let rgroup = imcu_height / self.min_dct_scaled_size as u32;
            let mut rows_left = (c.downsampled_height % imcu_height) as usize;
            if rows_left == 0 {
                rows_left = imcu_height as usize;
            }
            if ci == 0 {
                self.main.rowgroups_avail = ((rows_left as u32 - 1) / rgroup) + 1;
            }
            let base = self.main.xbase[ci];
            let xbuf = &mut self.main.xbuffer[w][ci];
            let last = xbuf[base + rows_left - 1];
            for i in 0..rgroup as usize * 2 {
                xbuf[base + rows_left + i] = last;
            }
        }
    }

    /// `set_wraparound_pointers`: the context rows at the top and bottom of the rolling buffer.
    fn set_wraparound_pointers(&mut self) {
        let m = self.min_dct_scaled_size as usize;
        for ci in 0..self.num_components as usize {
            let c = self.comp_info[ci];
            let rgroup = ((c.v_samp_factor * c.dct_h_scaled_size) / self.min_dct_scaled_size) as usize;
            let base = self.main.xbase[ci];
            for i in 0..rgroup {
                for w in 0..2 {
                    let xbuf = &mut self.main.xbuffer[w][ci];
                    xbuf[base + i - rgroup] = xbuf[base + rgroup * (m + 1) + i];
                    xbuf[base + rgroup * (m + 2) + i] = xbuf[base + i];
                }
            }
        }
    }

    /// `start_pass_main` (JBUF_PASS_THRU).
    pub(crate) fn start_pass_main(&mut self) -> Result<()> {
        if self.upsample.need_context_rows {
            self.main.context = true;
            self.make_funny_pointers();
            self.main.whichptr = 0;
            self.main.context_state = CtxState::PrepareForImcu;
            self.main.imcu_row_ctr = 0;
        } else {
            self.main.context = false;
        }
        self.main.buffer_full = false;
        self.main.rowgroup_ctr = 0;
        Ok(())
    }

    /// `jinit_d_main_controller` allocations: the row storage and (with context) the lists.
    pub(crate) fn init_main_buffers(&mut self) -> Result<()> {
        let m = self.min_dct_scaled_size as usize;
        if self.upsample.need_context_rows && self.min_dct_scaled_size < 2 {
            return Err(Error::NotImplemented);
        }
        let ngroups = if self.upsample.need_context_rows { m + 2 } else { m };
        if self.upsample.need_context_rows {
            self.alloc_funny_pointers();
        }
        self.main.buffer = Vec::with_capacity(self.num_components as usize);
        for ci in 0..self.num_components as usize {
            let c = self.comp_info[ci];
            let rgroup = ((c.v_samp_factor * c.dct_h_scaled_size) / self.min_dct_scaled_size) as usize;
            let width = (c.width_in_blocks as usize) * (c.dct_h_scaled_size as usize);
            self.main.buffer.push(vec![vec![0u8; width]; rgroup * ngroups]);
        }
        Ok(())
    }

    /// `process_data` dispatch: the `_process_data` method of the main controller.
    pub(crate) fn process_data_into(&mut self, scanlines: &mut [&mut [u8]]) -> Result<usize> {
        let mut out_row_ctr = 0usize;
        let out_rows_avail = scanlines.len();
        let mut buffer = std::mem::take(&mut self.main.buffer);
        let r = self.process_data_inner(&mut buffer, scanlines, &mut out_row_ctr, out_rows_avail);
        self.main.buffer = buffer;
        r?;
        Ok(out_row_ctr)
    }

    fn process_data_inner(
        &mut self,
        buffer: &mut [Vec<Vec<u8>>],
        scanlines: &mut [&mut [u8]],
        out_row_ctr: &mut usize,
        out_rows_avail: usize,
    ) -> Result<()> {
        if self.main.context {
            self.process_data_context(buffer, scanlines, out_row_ctr, out_rows_avail)
        } else {
            self.process_data_simple(buffer, scanlines, out_row_ctr, out_rows_avail)
        }
    }

    /// `process_data_simple_main`.
    fn process_data_simple(
        &mut self,
        buffer: &mut [Vec<Vec<u8>>],
        scanlines: &mut [&mut [u8]],
        out_row_ctr: &mut usize,
        out_rows_avail: usize,
    ) -> Result<()> {
        if !self.main.buffer_full {
            if !self.decompress_data_into(buffer, None)? {
                return Ok(());
            }
            self.main.buffer_full = true;
        }
        let rowgroups_avail = self.min_dct_scaled_size as u32;
        let identity: Vec<Vec<usize>> = (0..self.num_components as usize)
            .map(|ci| {
                let c = self.comp_info[ci];
                let rgroup = ((c.v_samp_factor * c.dct_h_scaled_size) / self.min_dct_scaled_size) as usize;
                (0..rgroup * self.min_dct_scaled_size as usize).collect()
            })
            .collect();
        let bases = vec![0usize; self.num_components as usize];
        let mut rg = self.main.rowgroup_ctr;
        self.upsample_pass(buffer, &identity, &bases, &mut rg, rowgroups_avail, scanlines, out_row_ctr, out_rows_avail)?;
        self.main.rowgroup_ctr = rg;
        if self.main.rowgroup_ctr >= rowgroups_avail {
            self.main.buffer_full = false;
            self.main.rowgroup_ctr = 0;
        }
        Ok(())
    }

    /// `process_data_context_main`.
    fn process_data_context(
        &mut self,
        buffer: &mut [Vec<Vec<u8>>],
        scanlines: &mut [&mut [u8]],
        out_row_ctr: &mut usize,
        out_rows_avail: usize,
    ) -> Result<()> {
        let m = self.min_dct_scaled_size as u32;
        if !self.main.buffer_full {
            let w = self.main.whichptr;
            let lists = self.main.xbuffer[w].clone();
            if !self.decompress_data_into(buffer, Some(&lists))? {
                return Ok(());
            }
            self.main.buffer_full = true;
            self.main.imcu_row_ctr += 1;
        }
        loop {
            match self.main.context_state {
                CtxState::PostponedRow => {
                    self.post_context(buffer, scanlines, out_row_ctr, out_rows_avail)?;
                    if self.main.rowgroup_ctr < self.main.rowgroups_avail {
                        return Ok(());
                    }
                    self.main.context_state = CtxState::PrepareForImcu;
                    if *out_row_ctr >= out_rows_avail {
                        return Ok(());
                    }
                    // fall through to PrepareForImcu
                    self.prepare_for_imcu(m)?;
                }
                CtxState::PrepareForImcu => {
                    self.prepare_for_imcu(m)?;
                }
                CtxState::ProcessImcu => {
                    self.post_context(buffer, scanlines, out_row_ctr, out_rows_avail)?;
                    if self.main.rowgroup_ctr < self.main.rowgroups_avail {
                        return Ok(());
                    }
                    if self.main.imcu_row_ctr == 1 {
                        self.set_wraparound_pointers();
                    }
                    self.main.whichptr ^= 1;
                    self.main.buffer_full = false;
                    self.main.rowgroup_ctr = m + 1;
                    self.main.rowgroups_avail = m + 2;
                    self.main.context_state = CtxState::PostponedRow;
                    return Ok(());
                }
            }
        }
    }

    /// The `CTX_PREPARE_FOR_IMCU` block of `process_data_context_main`.
    fn prepare_for_imcu(&mut self, m: u32) -> Result<()> {
        self.main.rowgroup_ctr = 0;
        self.main.rowgroups_avail = m - 1;
        if self.main.imcu_row_ctr == self.total_imcu_rows {
            self.set_bottom_pointers();
        }
        self.main.context_state = CtxState::ProcessImcu;
        Ok(())
    }

    /// The `_post_process_data` call of the context path: the xbuffer rows of the current
    /// pointer set go to the upsampler.
    fn post_context(
        &mut self,
        buffer: &mut [Vec<Vec<u8>>],
        scanlines: &mut [&mut [u8]],
        out_row_ctr: &mut usize,
        out_rows_avail: usize,
    ) -> Result<()> {
        let w = self.main.whichptr;
        let lists = self.main.xbuffer[w].clone();
        let avail = self.main.rowgroups_avail;
        let bases = self.main.xbase.clone();
        let mut rg = self.main.rowgroup_ctr;
        self.upsample_pass(buffer, &lists, &bases, &mut rg, avail, scanlines, out_row_ctr, out_rows_avail)?;
        self.main.rowgroup_ctr = rg;
        Ok(())
    }

    /// `decompress_data` dispatch for the main controller (`coef->_decompress_data`) with the
    /// rows of `lists` (or the plain buffer when `None`). Returns `false` on suspension.
    fn decompress_data_into(&mut self, buffer: &mut [Vec<Vec<u8>>], lists: Option<&[Vec<usize>]>) -> Result<bool> {
        let identity: Vec<Vec<usize>>;
        let lists: &[Vec<usize>] = match lists {
            Some(l) => l,
            None => {
                identity = (0..self.num_components as usize)
                    .map(|ci| {
                        let n = buffer[ci].len();
                        (0..n).collect()
                    })
                    .collect();
                &identity
            }
        };
        // The funny lists hold leading entries (negative C indices); the decoder writes from
        // the start of each list, i.e. from C index 0 (the `xbase` offset).
        let slices: Vec<&[usize]> = (0..self.num_components as usize)
            .map(|ci| &lists[ci][self.main.xbase.get(ci).copied().unwrap_or(0)..])
            .collect();
        let owned: Vec<Vec<usize>> = slices.iter().map(|s| s.to_vec()).collect();
        let mut out = OutPlanes { storage: buffer, lists: &owned };
        match self.decompress_onepass(&mut out)? {
            None => Ok(false),
            Some(ConsumeResult::RowCompleted) | Some(ConsumeResult::ScanCompleted) => Ok(true),
            Some(_) => Ok(true),
        }
    }

    /// `decompress_data` for raw output (`read_raw_data`): the planes are the caller's.
    pub(crate) fn decompress_data_raw(&mut self, planes: &mut [Vec<Vec<u8>>]) -> Result<bool> {
        let identity: Vec<Vec<usize>> = (0..self.num_components as usize)
            .map(|ci| (0..planes[ci].len()).collect())
            .collect();
        let mut out = OutPlanes { storage: planes, lists: &identity };
        Ok(self.decompress_onepass(&mut out)?.is_some())
    }
}

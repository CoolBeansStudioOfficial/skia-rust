// Port of: libjpeg-turbo src/jdphuff.c#L1-L641 (libjpeg_turbo@e14cbfaa, 3.1.0)
//
// Copyright (C) 1995-1997, Thomas G. Lane, Guido Vollbeding. libjpeg-turbo Modifications:
// Copyright (C) 2015-2016, 2019, D. R. Commander. Rust port Copyright (C) 2025 The skia-rust
// Authors. Licence: IJG (see LICENSE).
//
//! The progressive Huffman entropy decoder (`jdphuff.c`): the four scan kinds (DC first, AC
//! first, DC refinement, AC refinement) decode the coefficient blocks of one MCU in place.
//!
//! Each decoder works on a copy of the MCU's blocks supplied by the coefficient controller and
//! writes that copy back whether or not it succeeds, which is the effect of libjpeg writing
//! through the block pointers it is given, including the partial writes of a suspended MCU.

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

use std::rc::Rc;

use crate::Decompress;
use crate::error::{Error, Result};
use crate::huff::{BitPerm, JWRN_HUFF_BAD_CODE, get_bits, huff_extend};
use crate::srcio::Local;
use crate::tables::{DCTSIZE2, NATURAL_ORDER};

/// `JWRN_BOGUS_PROGRESSION` (index in `jerror.h`): a scan re-codes a coefficient inconsistently.
pub(crate) const JWRN_BOGUS_PROGRESSION: i32 = 119;

/// Which progressive decoder `start_pass_phuff_decoder` selected (`decode_mcu` pointer).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum PhuffKind {
    /// `decode_mcu_DC_first`.
    #[default]
    DcFirst,
    /// `decode_mcu_AC_first`.
    AcFirst,
    /// `decode_mcu_DC_refine`.
    DcRefine,
    /// `decode_mcu_AC_refine`.
    AcRefine,
}

impl Decompress {
    /// `start_pass_phuff_decoder`: checks the scan parameters, records the bit positions of the
    /// previous scan, selects the MCU decoder and builds the Huffman tables it needs.
    pub(crate) fn start_pass_phuff_decoder(&mut self) -> Result<()> {
        let is_dc_band = self.ss == 0;
        let mut bad = false;
        if is_dc_band {
            if self.se != 0 {
                bad = true;
            }
        } else {
            if self.ss > self.se || self.se >= DCTSIZE2 as i32 {
                bad = true;
            }
            // AC scans must be single-component (non-interleaved).
            if self.comps_in_scan != 1 {
                bad = true;
            }
        }
        if self.ah != 0 && self.al != self.ah - 1 {
            bad = true;
        }
        if self.al > 13 {
            bad = true;
        }
        if bad {
            return Err(Error::BadProgression);
        }
        let nc = self.num_components as usize;
        for ci in 0..self.comps_in_scan as usize {
            let cindex = self.comp_info
                [self.cur_comp_info[ci].ok_or(Error::Internal("scan component"))?]
            .component_index as usize;
            if !is_dc_band && self.coef_bits[cindex][0] < 0 {
                // AC without prior DC scan
                self.warn(JWRN_BOGUS_PROGRESSION);
            }
            for coefi in self.ss.min(1)..=self.se.max(9) {
                let coefi = coefi as usize;
                self.coef_bits[cindex + nc][coefi] = if self.input_scan_number > 1 {
                    self.coef_bits[cindex][coefi]
                } else {
                    0
                };
            }
            for coefi in self.ss..=self.se {
                let coefi = coefi as usize;
                let expected = if self.coef_bits[cindex][coefi] < 0 {
                    0
                } else {
                    self.coef_bits[cindex][coefi]
                };
                if self.ah != expected {
                    self.warn(JWRN_BOGUS_PROGRESSION);
                }
                self.coef_bits[cindex][coefi] = self.al;
            }
        }
        self.huff.phuff_kind = match (is_dc_band, self.ah == 0) {
            (true, true) => PhuffKind::DcFirst,
            (false, true) => PhuffKind::AcFirst,
            (true, false) => PhuffKind::DcRefine,
            (false, false) => PhuffKind::AcRefine,
        };
        for ci in 0..self.comps_in_scan as usize {
            let compptr = self.cur_comp_info[ci].ok_or(Error::Internal("scan component"))?;
            if is_dc_band {
                if self.ah == 0 {
                    // DC refinement needs no table.
                    let tbl = self.comp_info[compptr].dc_tbl_no as usize;
                    self.huff.dc_derived[tbl] = Some(Rc::new(self.make_derived(true, tbl)?));
                }
            } else {
                let tbl = self.comp_info[compptr].ac_tbl_no as usize;
                self.huff.ac_derived[tbl] = Some(Rc::new(self.make_derived(false, tbl)?));
            }
            self.huff.saved.last_dc_val[ci] = 0;
        }
        self.huff.bitstate = BitPerm::default();
        self.insufficient_data = false;
        self.huff.saved.eobrun = 0;
        self.huff.restarts_to_go = self.restart_interval;
        Ok(())
    }

    /// `process_restart` of the progressive decoder: also clears `EOBRUN`.
    fn phuff_process_restart(&mut self) -> Result<bool> {
        let bits = self.huff.bitstate.bits_left / 8;
        self.marker.discarded_bytes += bits as u32;
        self.huff.bitstate.bits_left = 0;
        if !self.read_restart_marker()? {
            return Ok(false);
        }
        for ci in 0..self.comps_in_scan as usize {
            self.huff.saved.last_dc_val[ci] = 0;
        }
        self.huff.saved.eobrun = 0;
        self.huff.restarts_to_go = self.restart_interval;
        if self.unread_marker == 0 {
            self.insufficient_data = false;
        }
        Ok(true)
    }

    /// Decodes one MCU of a progressive scan into `mcu` (`entropy->decode_mcu`).
    pub(crate) fn decode_mcu_progressive(&mut self, mcu: &mut [[i16; DCTSIZE2]]) -> Result<bool> {
        if self.restart_interval != 0
            && self.huff.restarts_to_go == 0
            && !self.phuff_process_restart()?
        {
            return Ok(false);
        }
        let ok = match self.huff.phuff_kind {
            PhuffKind::DcFirst => self.decode_mcu_dc_first(mcu)?,
            PhuffKind::AcFirst => self.decode_mcu_ac_first(mcu)?,
            PhuffKind::DcRefine => self.decode_mcu_dc_refine(mcu)?,
            PhuffKind::AcRefine => self.decode_mcu_ac_refine(mcu)?,
        };
        if !ok {
            return Ok(false);
        }
        if self.restart_interval != 0 {
            self.huff.restarts_to_go = self.huff.restarts_to_go.wrapping_sub(1);
        }
        Ok(true)
    }

    /// `decode_mcu_DC_first`.
    fn decode_mcu_dc_first(&mut self, mcu: &mut [[i16; DCTSIZE2]]) -> Result<bool> {
        let al = self.al;
        if self.insufficient_data {
            return Ok(true);
        }
        let mut br = Local {
            next: self.srcbuf.next,
            bytes: self.srcbuf.bytes_in_buffer,
        };
        let mut get_buffer = self.huff.bitstate.get_buffer;
        let mut bits_left = self.huff.bitstate.bits_left;
        let mut state = self.huff.saved;
        for blkn in 0..self.blocks_in_mcu as usize {
            let ci = self.mcu_membership[blkn] as usize;
            let compptr = self.cur_comp_info[ci].ok_or(Error::Internal("scan component"))?;
            let tbl_no = self.comp_info[compptr].dc_tbl_no as usize;
            let tbl = self.huff.dc_derived[tbl_no]
                .clone()
                .ok_or(Error::Internal("dc table"))?;
            let mut s = match self.huff_decode(&mut br, &mut get_buffer, &mut bits_left, &tbl)? {
                Some(v) => v,
                None => return self.decode_suspend(&br, get_buffer, bits_left),
            };
            if s != 0 {
                if !self.check_bit_buffer(&mut br, &mut get_buffer, &mut bits_left, s)? {
                    return self.decode_suspend(&br, get_buffer, bits_left);
                }
                let r = get_bits(get_buffer, &mut bits_left, s);
                s = huff_extend(r, s);
            }
            let last = state.last_dc_val[ci];
            if (last >= 0 && s > i32::MAX - last) || (last < 0 && s < i32::MIN - last) {
                return Err(Error::BadDctCoef);
            }
            s += last;
            state.last_dc_val[ci] = s;
            // (JCOEF)LEFT_SHIFT(s, Al)
            mcu[blkn][0] = (((s as u32) << al) as i32) as i16;
        }
        self.huff.bitstate = BitPerm {
            get_buffer,
            bits_left,
        };
        self.srcbuf.next = br.next;
        self.srcbuf.bytes_in_buffer = br.bytes;
        self.huff.saved = state;
        Ok(true)
    }

    /// `decode_mcu_AC_first`.
    fn decode_mcu_ac_first(&mut self, mcu: &mut [[i16; DCTSIZE2]]) -> Result<bool> {
        let se = self.se;
        let al = self.al;
        if self.insufficient_data {
            return Ok(true);
        }
        let mut eobrun = self.huff.saved.eobrun;
        if eobrun > 0 {
            // A band of zeroes: processed now (nothing to do).
            eobrun -= 1;
        } else {
            let mut br = Local {
                next: self.srcbuf.next,
                bytes: self.srcbuf.bytes_in_buffer,
            };
            let mut get_buffer = self.huff.bitstate.get_buffer;
            let mut bits_left = self.huff.bitstate.bits_left;
            let tbl = self.ac_table_of_scan()?;
            let mut k = self.ss;
            while k <= se {
                let mut s =
                    match self.huff_decode(&mut br, &mut get_buffer, &mut bits_left, &tbl)? {
                        Some(v) => v,
                        None => return self.decode_suspend(&br, get_buffer, bits_left),
                    };
                let mut r = s >> 4;
                s &= 15;
                if s != 0 {
                    k += r;
                    if !self.check_bit_buffer(&mut br, &mut get_buffer, &mut bits_left, s)? {
                        return self.decode_suspend(&br, get_buffer, bits_left);
                    }
                    r = get_bits(get_buffer, &mut bits_left, s);
                    s = huff_extend(r, s);
                    mcu[0][NATURAL_ORDER[k as usize]] = (((s as u32) << al) as i32) as i16;
                } else if r == 15 {
                    // ZRL: skip 15 zeroes in band.
                    k += 15;
                } else {
                    // EOBr, run length is 2^r + appended bits.
                    eobrun = 1u32 << r;
                    if r != 0 {
                        if !self.check_bit_buffer(&mut br, &mut get_buffer, &mut bits_left, r)? {
                            return self.decode_suspend(&br, get_buffer, bits_left);
                        }
                        r = get_bits(get_buffer, &mut bits_left, r);
                        eobrun = eobrun.wrapping_add(r as u32);
                    }
                    // This band is processed at this moment.
                    eobrun = eobrun.wrapping_sub(1);
                    break;
                }
                k += 1;
            }
            self.huff.bitstate = BitPerm {
                get_buffer,
                bits_left,
            };
            self.srcbuf.next = br.next;
            self.srcbuf.bytes_in_buffer = br.bytes;
        }
        self.huff.saved.eobrun = eobrun;
        Ok(true)
    }

    /// The AC table of a single-component AC scan (`entropy->ac_derived_tbl`).
    fn ac_table_of_scan(&self) -> Result<Rc<crate::huff::DTbl>> {
        let compptr = self.cur_comp_info[0].ok_or(Error::Internal("scan component"))?;
        let tbl_no = self.comp_info[compptr].ac_tbl_no as usize;
        self.huff.ac_derived[tbl_no]
            .clone()
            .ok_or(Error::Internal("ac table"))
    }

    /// `decode_mcu_DC_refine`: one correction bit per block.
    fn decode_mcu_dc_refine(&mut self, mcu: &mut [[i16; DCTSIZE2]]) -> Result<bool> {
        let p1 = 1i32 << self.al;
        let mut br = Local {
            next: self.srcbuf.next,
            bytes: self.srcbuf.bytes_in_buffer,
        };
        let mut get_buffer = self.huff.bitstate.get_buffer;
        let mut bits_left = self.huff.bitstate.bits_left;
        for blkn in 0..self.blocks_in_mcu as usize {
            if !self.check_bit_buffer(&mut br, &mut get_buffer, &mut bits_left, 1)? {
                return self.decode_suspend(&br, get_buffer, bits_left);
            }
            if get_bits(get_buffer, &mut bits_left, 1) != 0 {
                mcu[blkn][0] |= p1 as i16;
            }
        }
        self.huff.bitstate = BitPerm {
            get_buffer,
            bits_left,
        };
        self.srcbuf.next = br.next;
        self.srcbuf.bytes_in_buffer = br.bytes;
        Ok(true)
    }

    /// `decode_mcu_AC_refine`: newly nonzero coefficients and correction bits.
    fn decode_mcu_ac_refine(&mut self, mcu: &mut [[i16; DCTSIZE2]]) -> Result<bool> {
        let se = self.se;
        let p1 = 1i32 << self.al;
        // NEG_1 << Al
        let m1 = (-1i32) << self.al;
        if self.insufficient_data {
            return Ok(true);
        }
        let mut br = Local {
            next: self.srcbuf.next,
            bytes: self.srcbuf.bytes_in_buffer,
        };
        let mut get_buffer = self.huff.bitstate.get_buffer;
        let mut bits_left = self.huff.bitstate.bits_left;
        let mut eobrun = self.huff.saved.eobrun;
        let tbl = self.ac_table_of_scan()?;
        let mut newnz_pos: Vec<usize> = Vec::new();
        let block = &mut mcu[0];
        // `goto undoit` leaves the labelled block with `false`.
        let completed = 'body: {
            let mut k = self.ss;
            if eobrun == 0 {
                while k <= se {
                    let mut s =
                        match self.huff_decode(&mut br, &mut get_buffer, &mut bits_left, &tbl)? {
                            Some(v) => v,
                            None => break 'body false,
                        };
                    let mut r = s >> 4;
                    s &= 15;
                    if s != 0 {
                        if s != 1 {
                            // size of new coef should always be 1
                            self.warn(JWRN_HUFF_BAD_CODE);
                        }
                        if !self.check_bit_buffer(&mut br, &mut get_buffer, &mut bits_left, 1)? {
                            break 'body false;
                        }
                        // newly nonzero coef is positive, or negative
                        s = if get_bits(get_buffer, &mut bits_left, 1) != 0 {
                            p1
                        } else {
                            m1
                        };
                    } else if r != 15 {
                        // EOBr, run length is 2^r + appended bits
                        eobrun = 1u32 << r;
                        if r != 0 {
                            if !self.check_bit_buffer(
                                &mut br,
                                &mut get_buffer,
                                &mut bits_left,
                                r,
                            )? {
                                break 'body false;
                            }
                            r = get_bits(get_buffer, &mut bits_left, r);
                            eobrun = eobrun.wrapping_add(r as u32);
                        }
                        // rest of block is handled by EOB logic
                        break;
                    }
                    // Advance over already-nonzero coefs and r still-zero coefs, appending
                    // correction bits to the nonzeroes.
                    loop {
                        let pos = NATURAL_ORDER[k as usize];
                        if block[pos] != 0 {
                            if !self.check_bit_buffer(
                                &mut br,
                                &mut get_buffer,
                                &mut bits_left,
                                1,
                            )? {
                                break 'body false;
                            }
                            if get_bits(get_buffer, &mut bits_left, 1) != 0
                                && (i32::from(block[pos]) & p1) == 0
                            {
                                // do nothing if already set it
                                if block[pos] >= 0 {
                                    block[pos] = block[pos].wrapping_add(p1 as i16);
                                } else {
                                    block[pos] = block[pos].wrapping_add(m1 as i16);
                                }
                            }
                        } else {
                            r -= 1;
                            if r < 0 {
                                // reached target zero coefficient
                                break;
                            }
                        }
                        k += 1;
                        if k > se {
                            break;
                        }
                    }
                    if s != 0 {
                        let pos = NATURAL_ORDER[k as usize];
                        block[pos] = s as i16;
                        newnz_pos.push(pos);
                    }
                    k += 1;
                }
            }
            if eobrun > 0 {
                // Scan any remaining coefficient positions after the end-of-band.
                while k <= se {
                    let pos = NATURAL_ORDER[k as usize];
                    if block[pos] != 0 {
                        if !self.check_bit_buffer(&mut br, &mut get_buffer, &mut bits_left, 1)? {
                            break 'body false;
                        }
                        if get_bits(get_buffer, &mut bits_left, 1) != 0
                            && (i32::from(block[pos]) & p1) == 0
                        {
                            // do nothing if already changed it
                            if block[pos] >= 0 {
                                block[pos] = block[pos].wrapping_add(p1 as i16);
                            } else {
                                block[pos] = block[pos].wrapping_add(m1 as i16);
                            }
                        }
                    }
                    k += 1;
                }
                eobrun -= 1;
            }
            true
        };
        if !completed {
            // undoit: clear the new nonzero coefficients, return without saving the state.
            for &pos in newnz_pos.iter().rev() {
                block[pos] = 0;
            }
            return Ok(false);
        }
        self.huff.bitstate = BitPerm {
            get_buffer,
            bits_left,
        };
        self.srcbuf.next = br.next;
        self.srcbuf.bytes_in_buffer = br.bytes;
        self.huff.saved.eobrun = eobrun;
        Ok(true)
    }
}

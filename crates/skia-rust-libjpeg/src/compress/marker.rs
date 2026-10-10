// Port of: libjpeg-turbo src/jcmarker.c#L1-L670 (libjpeg_turbo@e14cbfaa, 3.1.0 source):
// `emit_marker`, `emit_2bytes`, `emit_dqt`, `emit_dht`, `emit_sof`, `emit_sos`, `emit_jfif_app0`,
// `emit_adobe_app14`, `write_marker_header`, `write_file_header`, `write_frame_header`,
// `write_scan_header` and `write_file_trailer`. The restart-interval marker (`emit_dri`) is not
// emitted because the restart interval is always zero.
//
// Copyright (C) 1991-1998, Thomas G. Lane. Modified 2009-2011 by D. R. Commander.
// Copyright (C) 2025 The skia-rust Authors.

// The C loop shapes over several parallel arrays are kept (`for i in 0..n`), as in libjpeg.
#![allow(clippy::needless_range_loop)]

use super::Compress;
use crate::error::{Error, Result};
use crate::tables::{ColorSpace, DCTSIZE2, NATURAL_ORDER};

/// `M_SOF0`: baseline DCT.
const M_SOF0: u8 = 0xC0;
/// `M_SOF1`: extended sequential DCT (used when a table number exceeds 1).
const M_SOF1: u8 = 0xC1;
/// `M_DHT`.
const M_DHT: u8 = 0xC4;
/// `M_SOI`.
pub(super) const M_SOI: u8 = 0xD8;
/// `M_EOI`.
pub(super) const M_EOI: u8 = 0xD9;
/// `M_SOS`.
const M_SOS: u8 = 0xDA;
/// `M_DQT`.
const M_DQT: u8 = 0xDB;
/// `M_APP0`.
const M_APP0: u8 = 0xE0;
/// `M_APP14`.
const M_APP14: u8 = 0xEE;

impl Compress {
    /// `emit_byte`.
    fn emit_byte(&mut self, val: u8) {
        self.output.push(val);
    }

    /// `emit_marker`.
    fn emit_marker(&mut self, mark: u8) {
        self.emit_byte(0xFF);
        self.emit_byte(mark);
    }

    /// `emit_2bytes`: a big-endian 16-bit value.
    fn emit_2bytes(&mut self, value: u32) {
        self.emit_byte(((value >> 8) & 0xFF) as u8);
        self.emit_byte((value & 0xFF) as u8);
    }

    /// `write_marker_header`: a marker and its length (the body length plus the two length bytes).
    pub(super) fn write_marker_header(&mut self, marker: u8, datalen: u32) -> Result<()> {
        if datalen > 65533 {
            return Err(Error::BadLength);
        }
        self.emit_marker(marker);
        self.emit_2bytes(datalen + 2);
        Ok(())
    }

    /// `emit_dqt`: the quantization table `index` (if not sent yet), in zigzag order. Returns 1
    /// when the table has 16-bit entries, as the C function does.
    fn emit_dqt(&mut self, index: usize) -> Result<u32> {
        let qtbl = self.quant_tbl_ptrs[index].ok_or(Error::NoQuantTable(index as i32))?;
        let mut prec = 0u32;
        for i in 0..DCTSIZE2 {
            if qtbl.quantval[i] > 255 {
                prec = 1;
            }
        }
        if !qtbl.sent_table {
            self.emit_marker(M_DQT);
            let len = if prec != 0 {
                DCTSIZE2 as u32 * 2 + 1 + 2
            } else {
                DCTSIZE2 as u32 + 1 + 2
            };
            self.emit_2bytes(len);
            self.emit_byte((index as u32 + (prec << 4)) as u8);
            for i in 0..DCTSIZE2 {
                let qval = u32::from(qtbl.quantval[NATURAL_ORDER[i]]);
                if prec != 0 {
                    self.emit_byte((qval >> 8) as u8);
                }
                self.emit_byte((qval & 0xFF) as u8);
            }
            if let Some(q) = self.quant_tbl_ptrs[index].as_mut() {
                q.sent_table = true;
            }
        }
        Ok(prec)
    }

    /// `emit_dht`: the Huffman table `index` (DC, or AC with `index + 0x10`) if not sent yet.
    fn emit_dht(&mut self, index: usize, is_ac: bool) -> Result<()> {
        let (htbl, marker_index) = if is_ac {
            (self.ac_huff_tbl_ptrs[index], index + 0x10)
        } else {
            (self.dc_huff_tbl_ptrs[index], index)
        };
        let htbl = htbl.ok_or(Error::NoHuffTable(marker_index as i32))?;
        if !htbl.sent_table {
            self.emit_marker(M_DHT);
            let mut length = 0usize;
            for i in 1..=16 {
                length += usize::from(htbl.bits[i]);
            }
            self.emit_2bytes((length + 2 + 1 + 16) as u32);
            self.emit_byte(marker_index as u8);
            for i in 1..=16 {
                self.emit_byte(htbl.bits[i]);
            }
            for i in 0..length {
                self.emit_byte(htbl.huffval[i]);
            }
            if is_ac {
                if let Some(h) = self.ac_huff_tbl_ptrs[index].as_mut() {
                    h.sent_table = true;
                }
            } else if let Some(h) = self.dc_huff_tbl_ptrs[index].as_mut() {
                h.sent_table = true;
            }
        }
        Ok(())
    }

    /// `emit_sof`: the frame header with the marker `code`.
    fn emit_sof(&mut self, code: u8) -> Result<()> {
        self.emit_marker(code);
        self.emit_2bytes(3 * self.num_components as u32 + 2 + 5 + 1);
        if self.image_height > 65535 || self.image_width > 65535 {
            return Err(Error::ImageTooBig);
        }
        self.emit_byte(self.data_precision as u8);
        self.emit_2bytes(self.image_height);
        self.emit_2bytes(self.image_width);
        self.emit_byte(self.num_components as u8);
        for ci in 0..self.num_components {
            let compptr = self.comp_info[ci];
            self.emit_byte(compptr.component_id as u8);
            self.emit_byte(((compptr.h_samp_factor << 4) + compptr.v_samp_factor) as u8);
            self.emit_byte(compptr.quant_tbl_no as u8);
        }
        Ok(())
    }

    /// `emit_sos`: the scan header for the components of the one scan (full spectral range,
    /// no successive approximation).
    fn emit_sos(&mut self) {
        self.emit_marker(M_SOS);
        self.emit_2bytes(2 * self.num_components as u32 + 2 + 1 + 3);
        self.emit_byte(self.num_components as u8);
        for i in 0..self.num_components {
            let compptr = self.comp_info[i];
            self.emit_byte(compptr.component_id as u8);
            let td = compptr.dc_tbl_no;
            let ta = compptr.ac_tbl_no;
            self.emit_byte(((td << 4) + ta) as u8);
        }
        // Ss = 0, Se = 63, Ah = Al = 0.
        self.emit_byte(0);
        self.emit_byte(63);
        self.emit_byte(0);
    }

    /// `emit_jfif_app0`.
    pub(super) fn emit_jfif_app0(&mut self) {
        self.emit_marker(M_APP0);
        self.emit_2bytes(2 + 4 + 1 + 2 + 1 + 2 + 2 + 1 + 1);
        for b in [0x4A, 0x46, 0x49, 0x46, 0] {
            self.emit_byte(b);
        }
        self.emit_byte(self.jfif_major_version);
        self.emit_byte(self.jfif_minor_version);
        self.emit_byte(self.density_unit);
        self.emit_2bytes(u32::from(self.x_density));
        self.emit_2bytes(u32::from(self.y_density));
        self.emit_byte(0);
        self.emit_byte(0);
    }

    /// `emit_adobe_app14`.
    pub(super) fn emit_adobe_app14(&mut self) {
        self.emit_marker(M_APP14);
        self.emit_2bytes(2 + 5 + 2 + 2 + 2 + 1);
        for b in [0x41, 0x64, 0x6F, 0x62, 0x65] {
            self.emit_byte(b);
        }
        self.emit_2bytes(100);
        self.emit_2bytes(0);
        self.emit_2bytes(0);
        let transform = match self.jpeg_color_space {
            ColorSpace::YCbCr => 1,
            ColorSpace::Ycck => 2,
            _ => 0,
        };
        self.emit_byte(transform);
    }

    /// `write_frame_header`: the quantization tables and the frame header. The marker is SOF0
    /// (baseline) unless a table number exceeds 1 or the precision is not 8 bits.
    pub(super) fn write_frame_header(&mut self) -> Result<()> {
        let mut prec = 0u32;
        for ci in 0..self.num_components {
            let qno = self.comp_info[ci].quant_tbl_no as usize;
            prec += self.emit_dqt(qno)?;
        }
        let mut is_baseline = self.data_precision == 8;
        if is_baseline {
            for ci in 0..self.num_components {
                let compptr = self.comp_info[ci];
                if compptr.dc_tbl_no > 1 || compptr.ac_tbl_no > 1 {
                    is_baseline = false;
                }
            }
            if prec != 0 && is_baseline {
                is_baseline = false;
            }
        }
        if is_baseline {
            self.emit_sof(M_SOF0)
        } else {
            self.emit_sof(M_SOF1)
        }
    }

    /// `write_scan_header`: the Huffman tables of the scan's components (not yet sent), then
    /// the scan header. No restart interval marker.
    pub(super) fn write_scan_header(&mut self) -> Result<()> {
        for i in 0..self.num_components {
            let compptr = self.comp_info[i];
            self.emit_dht(compptr.dc_tbl_no as usize, false)?;
            self.emit_dht(compptr.ac_tbl_no as usize, true)?;
        }
        self.emit_sos();
        Ok(())
    }
}

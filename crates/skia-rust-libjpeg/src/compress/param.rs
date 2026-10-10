// Port of: libjpeg-turbo src/jcparam.c#L1-L330 (libjpeg_turbo@e14cbfaa, 3.1.0 source), the parts
// Skia's encoder reaches: jpeg_add_quant_table, jpeg_set_linear_quality, jpeg_set_defaults,
// jpeg_default_colorspace and jpeg_set_colorspace. The standard tables are from jcmaster.c and
// jstdhuff.c (std_huff_tables).
//
// Copyright (C) 1991-1998, Thomas G. Lane. Modified 2003-2009 by Guido Vollbeding.
// Copyright (C) 2009-2011, 2016, 2025, D. R. Commander.
// Copyright (C) 2025 The skia-rust Authors.

use super::{Compress, GlobalState};
use crate::error::{Error, Result};
use crate::tables::{ColorSpace, DCTSIZE2, JHuffTbl, JQuantTbl, NUM_QUANT_TBLS};

/// `std_luminance_quant_tbl` (`jcparam.c`), in natural order.
const STD_LUMINANCE_QUANT_TBL: [u32; DCTSIZE2] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56,
    14, 17, 22, 29, 51, 87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113,
    92, 49, 64, 78, 87, 103, 121, 120, 101, 72, 92, 95, 98, 112, 100, 103, 99,
];

/// `std_chrominance_quant_tbl` (`jcparam.c`), in natural order.
const STD_CHROMINANCE_QUANT_TBL: [u32; DCTSIZE2] = [
    17, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26, 56, 99, 99, 99, 99, 99,
    47, 66, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
];

/// The standard DC luminance table (`bits_dc_luminance`, `val_dc_luminance` in jstdhuff.c).
const BITS_DC_LUMINANCE: [u8; 17] = [0, 0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];
const VAL_DC_LUMINANCE: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
/// The standard DC chrominance table.
const BITS_DC_CHROMINANCE: [u8; 17] = [0, 0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0];
const VAL_DC_CHROMINANCE: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
/// The standard AC luminance table.
const BITS_AC_LUMINANCE: [u8; 17] = [0, 0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7d];
const VAL_AC_LUMINANCE: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07,
    0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08, 0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0,
    0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28,
    0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
    0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
    0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7,
    0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5,
    0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2,
    0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];
/// The standard AC chrominance table.
const BITS_AC_CHROMINANCE: [u8; 17] = [0, 0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77];
const VAL_AC_CHROMINANCE: [u8; 162] = [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71,
    0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33, 0x52, 0xf0,
    0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34, 0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26,
    0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
    0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
    0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
    0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5,
    0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3,
    0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda,
    0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];

/// `add_huff_table` (`jstdhuff.c`) for the compressor: allocates the table if needed, then copies
/// the DHT-style `bits` and `huffval` into it, with `sent_table` cleared.
fn add_huff_table(slot: &mut Option<JHuffTbl>, bits: &[u8; 17], val: &[u8]) {
    let mut htbl = JHuffTbl {
        bits: *bits,
        huffval: [0; 256],
        sent_table: false,
    };
    htbl.huffval[..val.len()].copy_from_slice(val);
    *slot = Some(htbl);
}

impl Compress {
    /// `std_huff_tables` (`jstdhuff.c`): the standard tables in slots 0 (luminance) and 1
    /// (chrominance) of the DC and AC arrays.
    pub(super) fn std_huff_tables(&mut self) {
        add_huff_table(
            &mut self.dc_huff_tbl_ptrs[0],
            &BITS_DC_LUMINANCE,
            &VAL_DC_LUMINANCE,
        );
        add_huff_table(
            &mut self.ac_huff_tbl_ptrs[0],
            &BITS_AC_LUMINANCE,
            &VAL_AC_LUMINANCE,
        );
        add_huff_table(
            &mut self.dc_huff_tbl_ptrs[1],
            &BITS_DC_CHROMINANCE,
            &VAL_DC_CHROMINANCE,
        );
        add_huff_table(
            &mut self.ac_huff_tbl_ptrs[1],
            &BITS_AC_CHROMINANCE,
            &VAL_AC_CHROMINANCE,
        );
    }

    /// `jpeg_add_quant_table`: scales `basic_table` by `scale_factor` into table `which_tbl`.
    pub(super) fn add_quant_table(
        &mut self,
        which_tbl: usize,
        basic_table: &[u32; DCTSIZE2],
        scale_factor: i32,
        force_baseline: bool,
    ) -> Result<()> {
        self.check_state_start()?;
        if which_tbl >= NUM_QUANT_TBLS {
            return Err(Error::DqtIndex(which_tbl as i32));
        }
        let mut quantval = [0u16; DCTSIZE2];
        for i in 0..DCTSIZE2 {
            let mut temp = (i64::from(basic_table[i]) * i64::from(scale_factor) + 50) / 100;
            if temp <= 0 {
                temp = 1;
            }
            if temp > 32767 {
                temp = 32767;
            }
            if force_baseline && temp > 255 {
                temp = 255;
            }
            quantval[i] = temp as u16;
        }
        self.quant_tbl_ptrs[which_tbl] = Some(JQuantTbl {
            quantval,
            sent_table: false,
        });
        Ok(())
    }

    /// `jpeg_set_linear_quality`: both standard tables at `scale_factor` (percent).
    pub(super) fn set_linear_quality(
        &mut self,
        scale_factor: i32,
        force_baseline: bool,
    ) -> Result<()> {
        self.add_quant_table(0, &STD_LUMINANCE_QUANT_TBL, scale_factor, force_baseline)?;
        self.add_quant_table(1, &STD_CHROMINANCE_QUANT_TBL, scale_factor, force_baseline)
    }

    /// `jpeg_default_colorspace`: the file's colour space for the input colour space.
    pub(super) fn default_colorspace(&mut self) -> Result<()> {
        match self.in_color_space {
            ColorSpace::Grayscale => self.set_colorspace(ColorSpace::Grayscale),
            ColorSpace::Rgb
            | ColorSpace::ExtRgb
            | ColorSpace::ExtRgbx
            | ColorSpace::ExtBgr
            | ColorSpace::ExtBgrx
            | ColorSpace::ExtXbgr
            | ColorSpace::ExtXrgb
            | ColorSpace::ExtRgba
            | ColorSpace::ExtBgra
            | ColorSpace::ExtAbgr
            | ColorSpace::ExtArgb
            | ColorSpace::YCbCr => self.set_colorspace(ColorSpace::YCbCr),
            ColorSpace::Cmyk => self.set_colorspace(ColorSpace::Cmyk),
            ColorSpace::Ycck => self.set_colorspace(ColorSpace::Ycck),
            ColorSpace::Unknown => self.set_colorspace(ColorSpace::Unknown),
            ColorSpace::Rgb565 => Err(Error::BadColorspace),
        }
    }

    /// `jpeg_set_colorspace`: the component ids, sampling factors and table numbers of the file's
    /// colour space (`SET_COMP`), and the JFIF and Adobe markers it needs.
    pub(super) fn set_colorspace(&mut self, colorspace: ColorSpace) -> Result<()> {
        self.check_state_start()?;
        self.jpeg_color_space = colorspace;
        self.write_jfif_header = false;
        self.write_adobe_marker = false;
        // SET_COMP(index, id, hsamp, vsamp, quant, dctbl, actbl)
        let set =
            |c: &mut Self, index: usize, id: i32, h: i32, v: i32, q: i32, dc: i32, ac: i32| {
                let compptr = &mut c.comp_info[index];
                compptr.component_id = id;
                compptr.h_samp_factor = h;
                compptr.v_samp_factor = v;
                compptr.quant_tbl_no = q;
                compptr.dc_tbl_no = dc;
                compptr.ac_tbl_no = ac;
            };
        match colorspace {
            ColorSpace::Grayscale => {
                self.write_jfif_header = true;
                self.num_components = 1;
                set(self, 0, 1, 1, 1, 0, 0, 0);
            }
            ColorSpace::Rgb => {
                self.write_adobe_marker = true;
                self.num_components = 3;
                set(self, 0, 0x52, 1, 1, 0, 0, 0);
                set(self, 1, 0x47, 1, 1, 0, 0, 0);
                set(self, 2, 0x42, 1, 1, 0, 0, 0);
            }
            ColorSpace::YCbCr => {
                self.write_jfif_header = true;
                self.num_components = 3;
                set(self, 0, 1, 2, 2, 0, 0, 0);
                set(self, 1, 2, 1, 1, 1, 1, 1);
                set(self, 2, 3, 1, 1, 1, 1, 1);
            }
            ColorSpace::Cmyk => {
                self.write_adobe_marker = true;
                self.num_components = 4;
                set(self, 0, 0x43, 1, 1, 0, 0, 0);
                set(self, 1, 0x4D, 1, 1, 0, 0, 0);
                set(self, 2, 0x59, 1, 1, 0, 0, 0);
                set(self, 3, 0x4B, 1, 1, 0, 0, 0);
            }
            ColorSpace::Ycck => {
                self.write_adobe_marker = true;
                self.num_components = 4;
                set(self, 0, 1, 2, 2, 0, 0, 0);
                set(self, 1, 2, 1, 1, 1, 1, 1);
                set(self, 2, 3, 1, 1, 1, 1, 1);
                set(self, 3, 4, 2, 2, 0, 0, 0);
            }
            ColorSpace::Unknown => {
                self.num_components = self.input_components.max(0) as usize;
                if self.num_components < 1 || self.num_components > crate::tables::MAX_COMPONENTS {
                    return Err(Error::ComponentCount(
                        self.num_components as i32,
                        crate::tables::MAX_COMPONENTS as i32,
                    ));
                }
                for ci in 0..self.num_components {
                    set(self, ci, ci as i32, 1, 1, 0, 0, 0);
                }
            }
            _ => return Err(Error::BadColorspace),
        }
        Ok(())
    }

    /// `jpeg_set_quality`'s table step, with `q_scale_factor` kept for `jpeg_default_qtables`.
    pub(super) fn check_state_start(&self) -> Result<()> {
        if self.global_state == GlobalState::Start {
            Ok(())
        } else {
            Err(Error::BadState(self.global_state as i32))
        }
    }
}

// Port of: libjpeg-turbo src/jdmarker.c#L1-L1384 (libjpeg_turbo@e14cbfaa, 3.1.0 source)
//
// Copyright (C) 1991-1998, Thomas G. Lane. Modified 2009-2017 by Guido Vollbeding.
// libjpeg-turbo Modifications: Copyright (C) 2012-2017, 2021-2022, D. R. Commander.
// This file is part of the IJG JPEG software and is distributed under the IJG licence
// (see LICENSE). Rust port Copyright (C) 2025 The skia-rust Authors.
//
//! Marker reader: SOI/SOF/SOS/DHT/DQT/DRI/DAC, APPn and COM handling, restart markers.
//!
//! Each function returns `Ok(false)` where libjpeg returns `FALSE` (suspended on input) and
//! `Err` where libjpeg calls `ERREXIT`. Warnings are counted in `num_warnings`.

use crate::Decompress;
use crate::error::{Error, Result};
use crate::tables::{ColorSpace, JHuffTbl, JQuantTbl, NATURAL_ORDER};

// Marker codes (JPEG_MARKER in jdmarker.c).
pub(crate) const M_SOF0: i32 = 0xc0;
pub(crate) const M_SOF2: i32 = 0xc2;
pub(crate) const M_SOF3: i32 = 0xc3;
pub(crate) const M_SOF5: i32 = 0xc5;
pub(crate) const M_SOF6: i32 = 0xc6;
pub(crate) const M_SOF7: i32 = 0xc7;
pub(crate) const M_JPG: i32 = 0xc8;
pub(crate) const M_SOF9: i32 = 0xc9;
pub(crate) const M_SOF10: i32 = 0xca;
pub(crate) const M_SOF11: i32 = 0xcb;
pub(crate) const M_SOF13: i32 = 0xcd;
pub(crate) const M_SOF14: i32 = 0xce;
pub(crate) const M_SOF15: i32 = 0xcf;
pub(crate) const M_DHT: i32 = 0xc4;
pub(crate) const M_DAC: i32 = 0xcc;
pub(crate) const M_RST0: i32 = 0xd0;
pub(crate) const M_RST7: i32 = 0xd7;
pub(crate) const M_SOI: i32 = 0xd8;
pub(crate) const M_EOI: i32 = 0xd9;
pub(crate) const M_SOS: i32 = 0xda;
pub(crate) const M_DQT: i32 = 0xdb;
pub(crate) const M_DNL: i32 = 0xdc;
pub(crate) const M_DRI: i32 = 0xdd;
pub(crate) const M_APP0: i32 = 0xe0;
pub(crate) const M_APP14: i32 = 0xee;
pub(crate) const M_APP15: i32 = 0xef;
pub(crate) const M_COM: i32 = 0xfe;
pub(crate) const M_TEM: i32 = 0x01;

/// Return codes of `jpeg_consume_input` / `jpeg_read_header` (`JPEG_*` in `jpeglib.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsumeResult {
    /// `JPEG_SUSPENDED`: more input is needed; call again to resume.
    Suspended,
    /// `JPEG_REACHED_SOS`: the header of a scan has been read.
    ReachedSos,
    /// `JPEG_REACHED_EOI`: the end of the image has been read.
    ReachedEoi,
    /// `JPEG_ROW_COMPLETED`: a row of coefficients completed (input side).
    RowCompleted,
    /// `JPEG_SCAN_COMPLETED`: the last MCU row of a scan completed.
    ScanCompleted,
}

/// `jpeg_marker_parser_method`: the processor for a COM or APPn marker. `skip_variable` and
/// `get_interesting_appn` are the two defaults; `save_marker` is installed by
/// `jpeg_save_markers`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum MarkerProc {
    #[default]
    SkipVariable,
    GetInterestingAppn,
    SaveMarker,
}

/// `struct jpeg_marker_struct`: a saved marker. `original_length` is the length from the file,
/// `data` holds the first `data_length` bytes (the saved limit).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedMarker {
    /// The marker code (`M_APP1`, `M_COM`, ...).
    pub marker: u8,
    /// The segment length field, excluding the marker code bytes but including the length.
    pub original_length: u32,
    /// The saved data.
    pub data: Vec<u8>,
}

/// The marker reader's state (`my_marker_reader`).
#[derive(Debug, Clone, Default)]
pub(crate) struct MarkerReader {
    pub(crate) process_com: MarkerProc,
    pub(crate) length_limit_com: u32,
    pub(crate) process_appn: [MarkerProc; 16],
    pub(crate) length_limit_appn: [u32; 16],
    /// The saved marker being filled, when a SAVE_MARKERS segment suspended mid-way.
    pub(crate) cur_marker: Option<SavedMarker>,
    pub(crate) bytes_read: usize,
    pub(crate) next_restart_num: i32,
    pub(crate) saw_soi: bool,
    pub(crate) saw_sof: bool,
    pub(crate) discarded_bytes: u32,
}

/// Maps a marker code to its processor slot for `process_appn`.
fn appn_index(m: i32) -> usize {
    (m - M_APP0) as usize
}

impl Decompress {
    /// `get_soi`: start of image.
    fn get_soi(&mut self) -> Result<bool> {
        if self.marker.saw_soi {
            return Err(Error::SoiDuplicate);
        }
        // Reset the per-image defaults (libjpeg does this before any SOF).
        for i in 0..16 {
            self.arith_dc_l[i] = 0;
            self.arith_dc_u[i] = 1;
            self.arith_ac_k[i] = 5;
        }
        self.restart_interval = 0;
        self.jpeg_color_space = ColorSpace::Unknown;
        self.ccir601_sampling = false;
        self.saw_jfif_marker = false;
        self.jfif_major_version = 1;
        self.jfif_minor_version = 1;
        self.density_unit = 0;
        self.x_density = 1;
        self.y_density = 1;
        self.saw_adobe_marker = false;
        self.adobe_transform = 0;
        self.marker.saw_soi = true;
        Ok(true)
    }

    /// `get_sof`: start of frame (baseline, extended, progressive, arithmetic variants).
    fn get_sof(&mut self, is_prog: bool, is_arith: bool) -> Result<bool> {
        if self.marker.saw_sof {
            return Err(Error::SofDuplicate);
        }
        self.progressive_mode = is_prog;
        self.arith_code = is_arith;
        let mut l = self.input_vars();
        let Some(mut length) = self.input_2bytes(&mut l)? else { return Ok(false) };
        let Some(precision) = self.input_byte(&mut l)? else { return Ok(false) };
        self.data_precision = i32::from(precision);
        let Some(height) = self.input_2bytes(&mut l)? else { return Ok(false) };
        self.image_height = height as u32;
        let Some(width) = self.input_2bytes(&mut l)? else { return Ok(false) };
        self.image_width = width as u32;
        let Some(nc) = self.input_byte(&mut l)? else { return Ok(false) };
        self.num_components = i32::from(nc);
        length -= 8;
        if self.image_height == 0 || self.image_width == 0 || self.num_components <= 0 {
            return Err(Error::EmptyImage);
        }
        if length != self.num_components * 3 {
            return Err(Error::BadLength);
        }
        // comp_info is allocated once per image (alloc_small in libjpeg; kept if already set).
        if self.comp_info.len() != self.num_components as usize {
            self.comp_info = vec![crate::tables::CompInfo::default(); self.num_components as usize];
        }
        for ci in 0..self.num_components as usize {
            self.comp_info[ci].component_index = ci as i32;
            let Some(id) = self.input_byte(&mut l)? else { return Ok(false) };
            self.comp_info[ci].component_id = i32::from(id);
            let Some(c) = self.input_byte(&mut l)? else { return Ok(false) };
            let c = i32::from(c);
            self.comp_info[ci].h_samp_factor = (c >> 4) & 15;
            self.comp_info[ci].v_samp_factor = c & 15;
            let Some(q) = self.input_byte(&mut l)? else { return Ok(false) };
            self.comp_info[ci].quant_tbl_no = i32::from(q);
        }
        self.marker.saw_sof = true;
        self.input_sync(&l);
        Ok(true)
    }

    /// `get_sos`: start of scan.
    fn get_sos(&mut self) -> Result<bool> {
        if !self.marker.saw_sof {
            return Err(Error::SosNoSof);
        }
        let mut l = self.input_vars();
        let Some(length) = self.input_2bytes(&mut l)? else { return Ok(false) };
        let Some(n) = self.input_byte(&mut l)? else { return Ok(false) };
        let n = i32::from(n);
        if length != n * 2 + 6 || !(1..=4).contains(&n) {
            return Err(Error::BadLength);
        }
        self.comps_in_scan = n;
        self.cur_comp_info = [None; 4];
        for i in 0..n as usize {
            let Some(cc) = self.input_byte(&mut l)? else { return Ok(false) };
            let Some(c) = self.input_byte(&mut l)? else { return Ok(false) };
            let cc = i32::from(cc);
            let c = i32::from(c);
            let mut found = None;
            for ci in 0..(self.num_components as usize).min(4) {
                // libjpeg tests `!cinfo->cur_comp_info[ci]` with the SOF position as index.
                if cc == self.comp_info[ci].component_id && self.cur_comp_info[ci].is_none() {
                    found = Some(ci);
                    break;
                }
            }
            let Some(ci) = found else {
                return Err(Error::BadComponentId(cc));
            };
            self.cur_comp_info[i] = Some(ci);
            self.comp_info[ci].dc_tbl_no = (c >> 4) & 15;
            self.comp_info[ci].ac_tbl_no = c & 15;
            // Duplicate check against earlier entries of this scan (pi < i).
            for pi in 0..i {
                if self.cur_comp_info[pi] == Some(ci) {
                    return Err(Error::BadComponentId(cc));
                }
            }
        }
        let Some(ss) = self.input_byte(&mut l)? else { return Ok(false) };
        self.ss = i32::from(ss);
        let Some(se) = self.input_byte(&mut l)? else { return Ok(false) };
        self.se = i32::from(se);
        let Some(a) = self.input_byte(&mut l)? else { return Ok(false) };
        self.ah = (i32::from(a) >> 4) & 15;
        self.al = i32::from(a) & 15;
        self.marker.next_restart_num = 0;
        self.input_scan_number += 1;
        self.input_sync(&l);
        Ok(true)
    }

    /// `get_dac`: arithmetic coding conditioning tables (DAC).
    fn get_dac(&mut self) -> Result<bool> {
        let mut l = self.input_vars();
        let Some(length) = self.input_2bytes(&mut l)? else { return Ok(false) };
        let mut length = length - 2;
        while length > 0 {
            let Some(index) = self.input_byte(&mut l)? else { return Ok(false) };
            let Some(val) = self.input_byte(&mut l)? else { return Ok(false) };
            let index = i32::from(index);
            let val = i32::from(val);
            length -= 2;
            if !(0..32).contains(&index) {
                return Err(Error::DacIndex(index));
            }
            if index >= 16 {
                self.arith_ac_k[(index - 16) as usize] = val as u8;
            } else {
                self.arith_dc_l[index as usize] = (val & 0x0F) as u8;
                self.arith_dc_u[index as usize] = (val >> 4) as u8;
                if self.arith_dc_l[index as usize] > self.arith_dc_u[index as usize] {
                    return Err(Error::DacValue(val));
                }
            }
        }
        if length != 0 {
            return Err(Error::BadLength);
        }
        self.input_sync(&l);
        Ok(true)
    }

    /// `get_dht`: Huffman tables.
    fn get_dht(&mut self) -> Result<bool> {
        let mut l = self.input_vars();
        let Some(length) = self.input_2bytes(&mut l)? else { return Ok(false) };
        let mut length = length - 2;
        while length > 16 {
            let Some(index) = self.input_byte(&mut l)? else { return Ok(false) };
            let mut index = i32::from(index);
            let mut bits = [0u8; 17];
            let mut count: i32 = 0;
            for i in 1..=16 {
                let Some(b) = self.input_byte(&mut l)? else { return Ok(false) };
                bits[i] = b;
                count += i32::from(b);
            }
            length -= 1 + 16;
            if count > 256 || i64::from(count) > i64::from(length) {
                return Err(Error::BadHuffTable);
            }
            let mut huffval = [0u8; 256];
            for i in 0..count as usize {
                let Some(v) = self.input_byte(&mut l)? else { return Ok(false) };
                huffval[i] = v;
            }
            length -= count;
            let is_ac = index & 0x10 != 0;
            if is_ac {
                index -= 0x10;
            }
            if !(0..4).contains(&index) {
                return Err(Error::DhtIndex(index));
            }
            let tbl = JHuffTbl { bits, huffval, sent_table: false };
            if is_ac {
                self.ac_huff_tbl_ptrs[index as usize] = Some(tbl);
            } else {
                self.dc_huff_tbl_ptrs[index as usize] = Some(tbl);
            }
        }
        if length != 0 {
            return Err(Error::BadLength);
        }
        self.input_sync(&l);
        Ok(true)
    }

    /// `get_dqt`: quantization tables.
    fn get_dqt(&mut self) -> Result<bool> {
        let mut l = self.input_vars();
        let Some(length) = self.input_2bytes(&mut l)? else { return Ok(false) };
        let mut length = length - 2;
        while length > 0 {
            let Some(nb) = self.input_byte(&mut l)? else { return Ok(false) };
            let prec = i32::from(nb >> 4);
            let n = i32::from(nb & 0x0F);
            if n >= 4 {
                return Err(Error::DqtIndex(n));
            }
            let mut quantval = [0u16; 64];
            for i in 0..64 {
                let tmp = if prec != 0 {
                    let Some(v) = self.input_2bytes(&mut l)? else { return Ok(false) };
                    v as u16
                } else {
                    let Some(v) = self.input_byte(&mut l)? else { return Ok(false) };
                    u16::from(v)
                };
                quantval[NATURAL_ORDER[i]] = tmp;
            }
            self.quant_tbl_ptrs[n as usize] = Some(JQuantTbl { quantval, sent_table: false });
            length -= 64 + 1;
            if prec != 0 {
                length -= 64;
            }
        }
        if length != 0 {
            return Err(Error::BadLength);
        }
        self.input_sync(&l);
        Ok(true)
    }

    /// `get_dri`: restart interval.
    fn get_dri(&mut self) -> Result<bool> {
        let mut l = self.input_vars();
        let Some(length) = self.input_2bytes(&mut l)? else { return Ok(false) };
        if length != 4 {
            return Err(Error::BadLength);
        }
        let Some(tmp) = self.input_2bytes(&mut l)? else { return Ok(false) };
        self.restart_interval = tmp as u32;
        self.input_sync(&l);
        Ok(true)
    }

    /// `examine_app0`: JFIF (APP0) fields. Only the JFIF header is kept; JFXX is ignored.
    fn examine_app0(&mut self, data: &[u8], datalen: usize, remaining: i64) {
        let totallen = datalen as i64 + remaining;
        let _ = totallen;
        if datalen >= 14 && data[0] == 0x4A && data[1] == 0x46 && data[2] == 0x49 && data[3] == 0x46 && data[4] == 0 {
            self.saw_jfif_marker = true;
            self.jfif_major_version = data[5];
            self.jfif_minor_version = data[6];
            self.density_unit = data[7];
            self.x_density = (u16::from(data[8]) << 8) + u16::from(data[9]);
            self.y_density = (u16::from(data[10]) << 8) + u16::from(data[11]);
            if self.jfif_major_version != 1 {
                self.warn(JWRN_JFIF_MAJOR);
            }
        }
    }

    /// `examine_app14`: Adobe marker (transform flag).
    fn examine_app14(&mut self, data: &[u8], datalen: usize) {
        if datalen >= 12 && data[0] == 0x41 && data[1] == 0x64 && data[2] == 0x6F && data[3] == 0x62 && data[4] == 0x65 {
            let transform = data[11];
            self.saw_adobe_marker = true;
            self.adobe_transform = transform;
        }
    }

    /// `get_interesting_appn`: reads the first 14 bytes of an APP0/APP14 segment and skips
    /// the rest.
    fn get_interesting_appn(&mut self) -> Result<bool> {
        let mut l = self.input_vars();
        let Some(length) = self.input_2bytes(&mut l)? else { return Ok(false) };
        let mut length = i64::from(length) - 2;
        let mut b = [0u8; 14];
        let numtoread: usize = if length >= 14 {
            14
        } else if length > 0 {
            length as usize
        } else {
            0
        };
        for item in b.iter_mut().take(numtoread) {
            let Some(v) = self.input_byte(&mut l)? else { return Ok(false) };
            *item = v;
        }
        length -= numtoread as i64;
        match self.unread_marker {
            M_APP0 => self.examine_app0(&b, numtoread, length),
            M_APP14 => self.examine_app14(&b, numtoread),
            m => return Err(Error::UnknownMarker(m)),
        }
        self.input_sync(&l);
        if length > 0 {
            self.skip_input_data(length)?;
        }
        Ok(true)
    }

    /// `save_marker`: copies the whole segment into `marker_list` as it is read.
    fn save_marker(&mut self) -> Result<bool> {
        let mut l = self.input_vars();
        let mut length: i64 = 0;
        // bytes_read / data_length of the segment, as libjpeg's locals
        let mut bytes_read: usize;
        let data_length: usize;
        if self.marker.cur_marker.is_none() {
            let Some(len) = self.input_2bytes(&mut l)? else { return Ok(false) };
            length = i64::from(len) - 2;
            if length >= 0 {
                let mut limit = if self.unread_marker == M_COM {
                    self.marker.length_limit_com
                } else {
                    self.marker.length_limit_appn[appn_index(self.unread_marker)]
                } as i64;
                if length < limit {
                    limit = length;
                }
                let limit = limit as usize;
                self.marker.cur_marker = Some(SavedMarker {
                    marker: self.unread_marker as u8,
                    original_length: length as u32,
                    data: vec![0u8; limit],
                });
                self.marker.bytes_read = 0;
                bytes_read = 0;
                data_length = limit;
            } else {
                bytes_read = 0;
                data_length = 0;
            }
        } else {
            bytes_read = self.marker.bytes_read;
            data_length = self.marker.cur_marker.as_ref().map_or(0, |m| m.data.len());
        }
        while bytes_read < data_length {
            self.input_sync(&l);
            self.marker.bytes_read = bytes_read;
            if !self.make_byte_avail(&mut l)? {
                return Ok(false);
            }
            while bytes_read < data_length && l.bytes > 0 {
                let v = self.srcbuf.data[l.next];
                l.next += 1;
                l.bytes -= 1;
                if let Some(cur) = self.marker.cur_marker.as_mut() {
                    cur.data[bytes_read] = v;
                }
                bytes_read += 1;
            }
        }
        // The segment is complete.
        let mut completed = None;
        if let Some(cur) = self.marker.cur_marker.take() {
            length = i64::from(cur.original_length) - data_length as i64;
            completed = Some(cur);
        }
        let (data, data_len) = match &completed {
            Some(c) => (c.data.clone(), data_length),
            None => (Vec::new(), 0),
        };
        if let Some(c) = completed {
            self.marker_list.push(c);
        }
        match self.unread_marker {
            M_APP0 => self.examine_app0(&data, data_len, length),
            M_APP14 => self.examine_app14(&data, data_len),
            m => return Err(Error::UnknownMarker(m)),
        }
        self.input_sync(&l);
        if length > 0 {
            self.skip_input_data(length)?;
        }
        Ok(true)
    }

    /// `skip_variable`: skips a marker segment.
    fn skip_variable(&mut self) -> Result<bool> {
        let mut l = self.input_vars();
        let Some(length) = self.input_2bytes(&mut l)? else { return Ok(false) };
        let length = i64::from(length) - 2;
        self.input_sync(&l);
        if length > 0 {
            self.skip_input_data(length)?;
        }
        Ok(true)
    }

    /// `next_marker`: finds the next marker, skipping garbage and stuffed zero bytes.
    fn next_marker(&mut self) -> Result<bool> {
        let mut l = self.input_vars();
        let mut c: i32;
        loop {
            let Some(b) = self.input_byte(&mut l)? else { return Ok(false) };
            c = i32::from(b);
            // Skip any non-FF bytes (this may be an error, or garbage).
            while c != 0xFF {
                self.marker.discarded_bytes += 1;
                self.input_sync(&l);
                let Some(b) = self.input_byte(&mut l)? else { return Ok(false) };
                c = i32::from(b);
            }
            // Skip any duplicate FFs (fill bytes).
            loop {
                let Some(b) = self.input_byte(&mut l)? else { return Ok(false) };
                c = i32::from(b);
                if c != 0xFF {
                    break;
                }
            }
            if c != 0 {
                break;
            }
            // Stuffed zero: discard.
            self.marker.discarded_bytes += 2;
            self.input_sync(&l);
        }
        if self.marker.discarded_bytes != 0 {
            self.warn(JWRN_EXTRANEOUS_DATA);
            self.marker.discarded_bytes = 0;
        }
        self.unread_marker = c;
        self.input_sync(&l);
        Ok(true)
    }

    /// `first_marker`: the file must start with SOI.
    fn first_marker(&mut self) -> Result<bool> {
        let mut l = self.input_vars();
        let Some(c) = self.input_byte(&mut l)? else { return Ok(false) };
        let Some(c2) = self.input_byte(&mut l)? else { return Ok(false) };
        let (c, c2) = (i32::from(c), i32::from(c2));
        if c != 0xFF || c2 != M_SOI {
            return Err(Error::NoSoi(c, c2));
        }
        self.unread_marker = c2;
        self.input_sync(&l);
        Ok(true)
    }

    /// `read_markers`: the main marker loop. Returns the code libjpeg would return.
    pub(crate) fn read_markers(&mut self) -> Result<ConsumeResult> {
        loop {
            if self.unread_marker == 0 {
                if !self.marker.saw_soi {
                    if !self.first_marker()? {
                        return Ok(ConsumeResult::Suspended);
                    }
                } else if !self.next_marker()? {
                    return Ok(ConsumeResult::Suspended);
                }
            }
            match self.unread_marker {
                M_SOI => {
                    if !self.get_soi()? {
                        return Ok(ConsumeResult::Suspended);
                    }
                }
                M_SOF0 => {
                    if !self.get_sof(false, false)? {
                        return Ok(ConsumeResult::Suspended);
                    }
                }
                M_SOF2 => {
                    if !self.get_sof(true, false)? {
                        return Ok(ConsumeResult::Suspended);
                    }
                }
                // Lossless (SOF3, SOF11) is not ported; see crate docs.
                M_SOF3 | M_SOF11 => return Err(Error::NotImplemented),
                M_SOF9 => {
                    if !self.get_sof(false, true)? {
                        return Ok(ConsumeResult::Suspended);
                    }
                }
                M_SOF10 => {
                    if !self.get_sof(true, true)? {
                        return Ok(ConsumeResult::Suspended);
                    }
                }
                M_SOF5 | M_SOF6 | M_SOF7 | M_JPG | M_SOF13 | M_SOF14 | M_SOF15 => {
                    return Err(Error::SofUnsupported(self.unread_marker));
                }
                M_SOS => {
                    if !self.get_sos()? {
                        return Ok(ConsumeResult::Suspended);
                    }
                    self.unread_marker = 0;
                    return Ok(ConsumeResult::ReachedSos);
                }
                M_EOI => {
                    self.unread_marker = 0;
                    return Ok(ConsumeResult::ReachedEoi);
                }
                M_DAC => {
                    if !self.get_dac()? {
                        return Ok(ConsumeResult::Suspended);
                    }
                }
                M_DHT => {
                    if !self.get_dht()? {
                        return Ok(ConsumeResult::Suspended);
                    }
                }
                M_DQT => {
                    if !self.get_dqt()? {
                        return Ok(ConsumeResult::Suspended);
                    }
                }
                M_DRI => {
                    if !self.get_dri()? {
                        return Ok(ConsumeResult::Suspended);
                    }
                }
                m @ 0xe0..=0xef => {
                    let proc = self.marker.process_appn[appn_index(m)];
                    if !self.run_marker_proc(proc)? {
                        return Ok(ConsumeResult::Suspended);
                    }
                }
                M_COM => {
                    let proc = self.marker.process_com;
                    if !self.run_marker_proc(proc)? {
                        return Ok(ConsumeResult::Suspended);
                    }
                }
                m @ 0xd0..=0xd7 => {
                    // RSTn outside of a scan: ignored, as libjpeg does.
                    let _ = m;
                }
                M_TEM => {}
                M_DNL => {
                    if !self.skip_variable()? {
                        return Ok(ConsumeResult::Suspended);
                    }
                }
                m => return Err(Error::UnknownMarker(m)),
            }
            self.unread_marker = 0;
        }
    }

    /// Dispatches to a marker processor (`process_COM` / `process_APPn`).
    fn run_marker_proc(&mut self, proc: MarkerProc) -> Result<bool> {
        match proc {
            MarkerProc::SkipVariable => self.skip_variable(),
            MarkerProc::GetInterestingAppn => self.get_interesting_appn(),
            MarkerProc::SaveMarker => self.save_marker(),
        }
    }

    /// `read_restart_marker`: consumes the RSTn marker at the end of a restart interval.
    pub(crate) fn read_restart_marker(&mut self) -> Result<bool> {
        if self.unread_marker == 0 && !self.next_marker()? {
            return Ok(false);
        }
        if self.unread_marker == M_RST0 + self.marker.next_restart_num {
            // Correct restart marker.
            self.unread_marker = 0;
        } else {
            // Wrong marker: resynchronise.
            if !self.resync_to_restart(self.marker.next_restart_num)? {
                return Ok(false);
            }
        }
        self.marker.next_restart_num = (self.marker.next_restart_num + 1) & 7;
        Ok(true)
    }

    /// `jpeg_resync_to_restart` (the default `resync_to_restart` method).
    pub(crate) fn resync_to_restart(&mut self, desired: i32) -> Result<bool> {
        let mut marker = self.unread_marker;
        let mut action;
        self.warn(JWRN_MUST_RESYNC);
        loop {
            if marker < M_SOF0 {
                action = 2;
            } else if !(M_RST0..=M_RST7).contains(&marker) {
                action = 3;
            } else if marker == M_RST0 + ((desired + 1) & 7) || marker == M_RST0 + ((desired + 2) & 7) {
                action = 3;
            } else if marker == M_RST0 + ((desired - 1) & 7) || marker == M_RST0 + ((desired - 2) & 7) {
                action = 2;
            } else {
                action = 1;
            }
            match action {
                1 => {
                    // Discard marker and let entropy decoder resume processing.
                    self.unread_marker = 0;
                    return Ok(true);
                }
                2 => {
                    // Scan to the next marker, and repeat the test.
                    if !self.next_marker()? {
                        return Ok(false);
                    }
                    marker = self.unread_marker;
                }
                _ => {
                    // Return without advancing past this marker. The entropy decoder will
                    // be forced to process an empty segment.
                    return Ok(true);
                }
            }
        }
    }

    /// `reset_marker_reader`.
    pub(crate) fn reset_marker_reader(&mut self) {
        self.comp_info.clear();
        self.input_scan_number = 0;
        self.unread_marker = 0;
        self.marker.saw_soi = false;
        self.marker.saw_sof = false;
        self.marker.discarded_bytes = 0;
        self.marker.cur_marker = None;
    }

    /// `jinit_marker_reader`: default processors (skip everything except APP0/APP14).
    pub(crate) fn init_marker_reader(&mut self) {
        self.marker = MarkerReader::default();
        self.marker.process_com = MarkerProc::SkipVariable;
        self.marker.length_limit_com = 0;
        for i in 0..16 {
            self.marker.process_appn[i] = MarkerProc::SkipVariable;
            self.marker.length_limit_appn[i] = 0;
        }
        self.marker.process_appn[0] = MarkerProc::GetInterestingAppn;
        self.marker.process_appn[14] = MarkerProc::GetInterestingAppn;
        self.reset_marker_reader();
    }

    /// `jpeg_save_markers`: keep the segments for `marker_code` (or stop keeping them).
    pub fn save_markers(&mut self, marker_code: i32, length_limit: u32) -> Result<()> {
        let mut length_limit = length_limit;
        let processor;
        if length_limit != 0 {
            processor = MarkerProc::SaveMarker;
            // Make sure the APP0/APP14 data we examine is saved in full.
            if marker_code == M_APP0 && length_limit < 14 {
                length_limit = 14;
            } else if marker_code == M_APP14 && length_limit < 12 {
                length_limit = 12;
            }
        } else {
            processor = MarkerProc::SkipVariable;
            if marker_code == M_APP0 || marker_code == M_APP14 {
                // Keep examining JFIF and Adobe data even when not saving.
                return self.set_marker_processor(marker_code, MarkerProc::GetInterestingAppn, 0);
            }
        }
        self.set_marker_processor(marker_code, processor, length_limit)
    }

    fn set_marker_processor(&mut self, marker_code: i32, proc: MarkerProc, limit: u32) -> Result<()> {
        if marker_code == M_COM {
            self.marker.process_com = proc;
            self.marker.length_limit_com = limit;
        } else if (M_APP0..=M_APP15).contains(&marker_code) {
            self.marker.process_appn[appn_index(marker_code)] = proc;
            self.marker.length_limit_appn[appn_index(marker_code)] = limit;
        } else {
            return Err(Error::UnknownMarker(marker_code));
        }
        Ok(())
    }

    /// Records a warning (`WARNMS*`). Warnings never stop decoding.
    pub(crate) fn warn(&mut self, code: i32) {
        self.num_warnings += 1;
        self.last_warning = code;
    }

}

/// `JWRN_JFIF_MAJOR`.
pub(crate) const JWRN_JFIF_MAJOR: i32 = 1;
/// `JWRN_EXTRANEOUS_DATA`.
pub(crate) const JWRN_EXTRANEOUS_DATA: i32 = 2;
/// `JWRN_MUST_RESYNC`.
pub(crate) const JWRN_MUST_RESYNC: i32 = 3;
/// `JWRN_ADOBE_XFORM`.
pub(crate) const JWRN_ADOBE_XFORM: i32 = 4;

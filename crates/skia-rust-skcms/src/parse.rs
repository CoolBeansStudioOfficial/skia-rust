// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skcms/skcms.cc

//! ICC profile parsing.

use std::sync::Arc;

use crate::curve::MAX_TABLE_ENTRIES;
use crate::math::fabsf_;
use crate::public::{
    A2B, B2A, ByteView, Cicp, Curve, Hagc, IccProfile, Matrix3x3, Matrix3x4, TransferFunction,
    signature,
};

// A potential vulnerability exists where a large CLUT can cause an integer
// overflow in skcms's transformation logic. Limit the total number of grid
// points to a safe value. 350 million ensures that 6 * index will not overflow
// a 32-bit signed integer (which is what AVX2/AVX-512 gather expects).
// Port of: modules/skcms/skcms.cc#L42 (chrome/m156)
const SKCMS_MAX_GRID_POINTS: u64 = 350_000_000;

// Additional ICC signature values that are only used internally
// Port of: modules/skcms/skcms.cc#L351-L385 (chrome/m156)
mod sig {
    // File signature
    pub const ACSP: u32 = 0x6163_7370;

    // Tag signatures
    pub const R_TRC: u32 = 0x7254_5243;
    pub const G_TRC: u32 = 0x6754_5243;
    pub const B_TRC: u32 = 0x6254_5243;
    pub const K_TRC: u32 = 0x6B54_5243;

    pub const R_XYZ: u32 = 0x7258_595A;
    pub const G_XYZ: u32 = 0x6758_595A;
    pub const B_XYZ: u32 = 0x6258_595A;

    pub const A2B0: u32 = 0x4132_4230;
    pub const B2A0: u32 = 0x4232_4130;

    pub const CHAD: u32 = 0x6368_6164;
    pub const WTPT: u32 = 0x7774_7074;

    pub const CICP: u32 = 0x6369_6370;
    pub const HAGC: u32 = 0x4841_4743;

    // Type signatures
    pub const CURV: u32 = 0x6375_7276;
    pub const HAGC_TYPE: u32 = 0x6861_6763;
    pub const MFT1: u32 = 0x6D66_7431;
    pub const MFT2: u32 = 0x6D66_7432;
    pub const MAB: u32 = 0x6D41_4220;
    pub const MBA: u32 = 0x6D42_4120;
    pub const PARA: u32 = 0x7061_7261;
    pub const SF32: u32 = 0x7366_3332;
}

// Sizes of the in-memory layouts (`header_Layout` and friends) that the C++ maps over the buffer.
const HEADER_LAYOUT_SIZE: u64 = 132;
const TAG_LAYOUT_SIZE: u64 = 12;
const SF32_LAYOUT_SIZE: u64 = 44;
const XYZ_LAYOUT_SIZE: u64 = 20;
const PARA_LAYOUT_FIXED_SIZE: u64 = 12;
const CURV_LAYOUT_FIXED_SIZE: u64 = 12;
const MFT1_LAYOUT_FIXED_SIZE: u64 = 48;
const MFT2_LAYOUT_FIXED_SIZE: u64 = 52;
const MAB_LAYOUT_SIZE: u64 = 32;
const CLUT_LAYOUT_FIXED_SIZE: u64 = 20;
const CICP_LAYOUT_SIZE: u64 = 12;
const HAGC_LAYOUT_SIZE: u64 = 12;

/// One entry of an ICC profile's tag table: where the tag's data lives in the profile buffer.
// Port of: modules/skcms/src/skcms_internals.h#L131-L136 (chrome/m156)
#[doc(alias = "skcms_ICCTag")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IccTag {
    pub signature: u32,
    pub type_: u32,
    pub size: u32,
    /// Offset of the tag's data from the start of the profile buffer (`buf` in skcms).
    pub offset: usize,
}

// Big-endian reads. The C++ trusts that its earlier validation keeps reads inside the buffer;
// out-of-range bytes read as zero here instead of being undefined behavior.
// Port of: modules/skcms/skcms.cc#L387-L413 (chrome/m156)
fn byte_at(buf: &[u8], off: usize) -> u8 {
    buf.get(off).copied().unwrap_or(0)
}

fn read_big_u16(buf: &[u8], off: usize) -> u16 {
    u16::from_be_bytes([byte_at(buf, off), byte_at(buf, off.wrapping_add(1))])
}

fn read_big_u32(buf: &[u8], off: usize) -> u32 {
    u32::from_be_bytes([
        byte_at(buf, off),
        byte_at(buf, off.wrapping_add(1)),
        byte_at(buf, off.wrapping_add(2)),
        byte_at(buf, off.wrapping_add(3)),
    ])
}

#[allow(clippy::cast_possible_wrap)] // mirrors (int32_t)read_big_u32
fn read_big_i32(buf: &[u8], off: usize) -> i32 {
    read_big_u32(buf, off) as i32
}

#[allow(clippy::cast_precision_loss)] // mirrors static_cast<float>(int32_t)
fn read_big_fixed(buf: &[u8], off: usize) -> f32 {
    read_big_i32(buf, off) as f32 * (1.0f32 / 65536.0f32)
}

#[allow(clippy::cast_possible_truncation)] // mirrors the C++ casts of the ported arithmetic
fn get_tag_table_entry(profile: &IccProfile, idx: u32) -> Option<IccTag> {
    let buf = profile.buffer.as_ref()?;
    let entry = HEADER_LAYOUT_SIZE as usize + idx as usize * TAG_LAYOUT_SIZE as usize;
    let sig = read_big_u32(buf, entry);
    let offset = read_big_u32(buf, entry + 4) as usize;
    let size = read_big_u32(buf, entry + 8);
    Some(IccTag {
        signature: sig,
        type_: read_big_u32(buf, offset),
        size,
        offset,
    })
}

/// Looks up the `idx`-th tag of `profile`.
// Port of: modules/skcms/skcms.cc#L1352-L1360 (chrome/m156)
#[doc(alias = "skcms_GetTagByIndex")]
#[must_use]
pub fn get_tag_by_index(profile: &IccProfile, idx: u32) -> Option<IccTag> {
    profile.buffer.as_ref()?;
    if idx >= profile.tag_count {
        return None;
    }
    get_tag_table_entry(profile, idx)
}

/// Looks up the tag with signature `sig` in `profile`.
// Port of: modules/skcms/skcms.cc#L1362-L1375 (chrome/m156)
#[doc(alias = "skcms_GetTagBySignature")]
#[must_use]
#[allow(clippy::cast_possible_truncation)] // mirrors the C++ casts of the ported arithmetic
pub fn get_tag_by_signature(profile: &IccProfile, sig: u32) -> Option<IccTag> {
    let buf = profile.buffer.as_ref()?;
    for i in 0..profile.tag_count {
        let entry = HEADER_LAYOUT_SIZE as usize + i as usize * TAG_LAYOUT_SIZE as usize;
        if read_big_u32(buf, entry) == sig {
            return get_tag_table_entry(profile, i);
        }
    }
    None
}

/// Reads the profile's 'chad' (chromatic adaptation) tag.
// Port of: modules/skcms/skcms.cc#L459-L476 (chrome/m156)
#[doc(alias = "skcms_GetCHAD")]
#[must_use]
pub fn get_chad(profile: &IccProfile) -> Option<Matrix3x3> {
    let tag = get_tag_by_signature(profile, sig::CHAD)?;

    if tag.type_ != sig::SF32 || u64::from(tag.size) < SF32_LAYOUT_SIZE {
        return None;
    }

    let buf = profile.buffer.as_ref()?;
    let mut m = Matrix3x3::default();
    let mut values = tag.offset + 8;
    for r in 0..3 {
        for c in 0..3 {
            m.vals[r][c] = read_big_fixed(buf, values);
            values += 4;
        }
    }
    Some(m)
}

// Port of: modules/skcms/skcms.cc#L488-L499 (chrome/m156)
fn read_tag_xyz(buf: &[u8], tag: &IccTag) -> Option<(f32, f32, f32)> {
    if tag.type_ != signature::XYZ || u64::from(tag.size) < XYZ_LAYOUT_SIZE {
        return None;
    }

    let x = read_big_fixed(buf, tag.offset + 8);
    let y = read_big_fixed(buf, tag.offset + 12);
    let z = read_big_fixed(buf, tag.offset + 16);
    Some((x, y, z))
}

/// Reads the profile's 'wtpt' (media white point) tag.
// Port of: modules/skcms/skcms.cc#L501-L505 (chrome/m156)
#[doc(alias = "skcms_GetWTPT")]
#[must_use]
pub fn get_wtpt(profile: &IccProfile) -> Option<[f32; 3]> {
    let tag = get_tag_by_signature(profile, sig::WTPT)?;
    let buf = profile.buffer.as_ref()?;
    let (x, y, z) = read_tag_xyz(buf, &tag)?;
    Some([x, y, z])
}

// Port of: modules/skcms/skcms.cc#L507-L536 (chrome/m156)
#[allow(clippy::match_same_arms)] // one arm per op/format, as in the C++ switch
fn data_color_space_channel_count(data_color_space: u32) -> i32 {
    match data_color_space {
        signature::CMYK => 4,
        signature::GRAY => 1,
        signature::RGB
        | signature::LAB
        | signature::XYZ
        | signature::CIELUV
        | signature::YCBCR
        | signature::CIEYXY
        | signature::HSV
        | signature::HLS
        | signature::CMY
        | signature::CLR3 => 3,
        signature::CLR2 => 2,
        signature::CLR4 => 4,
        signature::CLR5 => 5,
        signature::CLR6 => 6,
        signature::CLR7 => 7,
        signature::CLR8 => 8,
        signature::CLR9 => 9,
        signature::CLR10 => 10,
        signature::CLR11 => 11,
        signature::CLR12 => 12,
        signature::CLR13 => 13,
        signature::CLR14 => 14,
        signature::CLR15 => 15,
        _ => -1,
    }
}

/// Returns the number of channels of input data that are expected on the "A" side of the
/// profile. This is useful for image codecs, where the image data and the accompanying profile
/// might have conflicting data shapes. In some cases, the result is unclear or invalid. In that
/// case, the function will return a negative value to signal an error.
// Port of: modules/skcms/skcms.cc#L538-L568 (chrome/m156)
#[doc(alias = "skcms_GetInputChannelCount")]
#[must_use]
pub fn get_input_channel_count(profile: &IccProfile) -> i32 {
    let mut a2b_count = 0;
    if profile.has_a2b {
        a2b_count = if profile.a2b.input_channels != 0 {
            // At most 4, so no truncation.
            i32::try_from(profile.a2b.input_channels).unwrap_or(i32::MAX)
        } else {
            3
        };
    }

    let mut trc_count = 0;
    if get_tag_by_signature(profile, sig::K_TRC).is_some() {
        trc_count = 1;
    } else if profile.has_trc {
        trc_count = 3;
    }

    let dcs_count = data_color_space_channel_count(profile.data_color_space);

    if dcs_count < 0 {
        return -1;
    }

    if a2b_count > 0 && a2b_count != dcs_count {
        return -1;
    }
    if trc_count > 0 && trc_count != dcs_count {
        return -1;
    }

    dcs_count
}

// Port of: modules/skcms/skcms.cc#L570-L575 (chrome/m156)
fn read_to_xyzd50(buf: &[u8], r_xyz: &IccTag, g_xyz: &IccTag, b_xyz: &IccTag) -> Option<Matrix3x3> {
    let mut to_xyz = Matrix3x3::default();
    let (x, y, z) = read_tag_xyz(buf, r_xyz)?;
    to_xyz.vals[0][0] = x;
    to_xyz.vals[1][0] = y;
    to_xyz.vals[2][0] = z;
    let (x, y, z) = read_tag_xyz(buf, g_xyz)?;
    to_xyz.vals[0][1] = x;
    to_xyz.vals[1][1] = y;
    to_xyz.vals[2][1] = z;
    let (x, y, z) = read_tag_xyz(buf, b_xyz)?;
    to_xyz.vals[0][2] = x;
    to_xyz.vals[1][2] = y;
    to_xyz.vals[2][2] = z;
    Some(to_xyz)
}

// Port of: modules/skcms/skcms.cc#L585-L652 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // mirrors the C++ casts of the ported arithmetic
#[allow(clippy::items_after_statements)] // mirrors the C++ local declarations
fn read_curve_para(buf: &[u8], off: usize, size: u32) -> Option<(Curve, u32)> {
    if u64::from(size) < PARA_LAYOUT_FIXED_SIZE {
        return None;
    }

    const K_G: u16 = 0;
    const K_GAB: u16 = 1;
    const K_GABC: u16 = 2;
    const K_GABCD: u16 = 3;
    const K_GABCDEF: u16 = 4;
    let function_type = read_big_u16(buf, off + 8);
    if function_type > K_GABCDEF {
        return None;
    }

    const CURVE_BYTES: [u32; 5] = [4, 12, 16, 20, 28];
    if u64::from(size) < PARA_LAYOUT_FIXED_SIZE + u64::from(CURVE_BYTES[function_type as usize]) {
        return None;
    }

    let curve_size = 12 + CURVE_BYTES[function_type as usize];

    let variable = off + PARA_LAYOUT_FIXED_SIZE as usize;
    let mut p = TransferFunction {
        g: 0.0,
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 0.0,
        e: 0.0,
        f: 0.0,
    };
    p.g = read_big_fixed(buf, variable);

    #[allow(clippy::float_cmp)] // mirrors the C++ exact zero tests
    match function_type {
        K_GAB => {
            p.a = read_big_fixed(buf, variable + 4);
            p.b = read_big_fixed(buf, variable + 8);
            if p.a == 0.0 {
                return None;
            }
            p.d = -p.b / p.a;
        }
        K_GABC => {
            p.a = read_big_fixed(buf, variable + 4);
            p.b = read_big_fixed(buf, variable + 8);
            p.e = read_big_fixed(buf, variable + 12);
            if p.a == 0.0 {
                return None;
            }
            p.d = -p.b / p.a;
            p.f = p.e;
        }
        K_GABCD => {
            p.a = read_big_fixed(buf, variable + 4);
            p.b = read_big_fixed(buf, variable + 8);
            p.c = read_big_fixed(buf, variable + 12);
            p.d = read_big_fixed(buf, variable + 16);
        }
        K_GABCDEF => {
            p.a = read_big_fixed(buf, variable + 4);
            p.b = read_big_fixed(buf, variable + 8);
            p.c = read_big_fixed(buf, variable + 12);
            p.d = read_big_fixed(buf, variable + 16);
            p.e = read_big_fixed(buf, variable + 20);
            p.f = read_big_fixed(buf, variable + 24);
        }
        _ => {
            let _ = K_G;
        }
    }
    if p.is_srgbish() {
        Some((Curve::Parametric(p), curve_size))
    } else {
        None
    }
}

// Port of: modules/skcms/skcms.cc#L664-L705 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // mirrors the C++ casts of the ported arithmetic
fn read_curve_curv(shared: &Arc<[u8]>, off: usize, size: u32) -> Option<(Curve, u32)> {
    if u64::from(size) < CURV_LAYOUT_FIXED_SIZE {
        return None;
    }

    let value_count = read_big_u32(shared, off + 8);
    if u64::from(size) < CURV_LAYOUT_FIXED_SIZE + u64::from(value_count) * 2 {
        return None;
    }

    // At most 12 + 2 * (2^32 - 1), which fits in u32 only when the check above bounds it by
    // `size`; it does, since size fits in u32.
    let curve_size = u32::try_from(CURV_LAYOUT_FIXED_SIZE + u64::from(value_count) * 2).ok()?;

    let variable = off + CURV_LAYOUT_FIXED_SIZE as usize;
    if value_count < 2 {
        let mut tf = TransferFunction {
            g: 0.0,
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 0.0,
            e: 0.0,
            f: 0.0,
        };
        if value_count == 0 {
            // Empty tables are a shorthand for an identity curve
            tf.g = 1.0;
        } else {
            // Single entry tables are a shorthand for simple gamma
            tf.g = f32::from(read_big_u16(shared, variable)) * (1.0f32 / 256.0f32);
        }
        return Some((Curve::Parametric(tf), curve_size));
    }
    if value_count > MAX_TABLE_ENTRIES {
        return None;
    }
    Some((
        Curve::Table16 {
            entries: value_count,
            table: ByteView::new(Arc::clone(shared), variable),
        },
        curve_size,
    ))
}

// Parses both curveType and parametricCurveType data. Ensures that at most 'size' bytes are read.
// Returns the curve and the number of bytes it used.
// Port of: modules/skcms/skcms.cc#L709-L723 (chrome/m156)
fn read_curve(shared: &Arc<[u8]>, off: usize, size: u32) -> Option<(Curve, u32)> {
    if size < 4 {
        return None;
    }

    let type_ = read_big_u32(shared, off);
    if type_ == sig::PARA {
        read_curve_para(shared, off, size)
    } else if type_ == sig::CURV {
        read_curve_curv(shared, off, size)
    } else {
        None
    }
}

// The operations A2B and B2A share when reading `mft1` and `mft2` tags.
trait MftTarget {
    fn read_mft_common(&mut self, buf: &[u8], tag_off: usize) -> bool;
    fn input_channels(&self) -> u32;
    fn output_channels(&self) -> u32;
    fn grid_points(&self) -> [u8; 4];
    fn set_input_curve(&mut self, i: usize, curve: Curve);
    fn set_output_curve(&mut self, i: usize, curve: Curve);
    fn set_grid(&mut self, grid_8: Option<ByteView>, grid_16: Option<ByteView>);
}

impl MftTarget for A2B {
    // Port of: modules/skcms/skcms.cc#L750-L778 (chrome/m156)
    fn read_mft_common(&mut self, buf: &[u8], tag_off: usize) -> bool {
        let a2b = self;
        // MFT matrices are applied before the first set of curves, but must be identity unless the
        // input is PCSXYZ. We don't support PCSXYZ profiles, so we ignore this matrix. Note that the
        // matrix in skcms_A2B is applied later in the pipe, so supporting this would require another
        // field/flag.
        a2b.matrix_channels = 0;
        a2b.input_channels = u32::from(byte_at(buf, tag_off + 8));
        a2b.output_channels = u32::from(byte_at(buf, tag_off + 9));

        // We require exactly three (ie XYZ/Lab/RGB) output channels
        if a2b.output_channels as usize != a2b.output_curves.len() {
            return false;
        }
        // We require at least one, and no more than four (ie CMYK) input channels
        if a2b.input_channels < 1 || a2b.input_channels as usize > a2b.input_curves.len() {
            return false;
        }

        let mut total_grid_points: u64 = 1;
        for i in 0..a2b.input_channels as usize {
            a2b.grid_points[i] = byte_at(buf, tag_off + 10);
            total_grid_points *= u64::from(a2b.grid_points[i]);
        }
        // The grid only makes sense with at least two points along each axis
        if a2b.grid_points[0] < 2 || total_grid_points > SKCMS_MAX_GRID_POINTS {
            return false;
        }
        true
    }
    fn input_channels(&self) -> u32 {
        self.input_channels
    }
    fn output_channels(&self) -> u32 {
        self.output_channels
    }
    fn grid_points(&self) -> [u8; 4] {
        self.grid_points
    }
    fn set_input_curve(&mut self, i: usize, curve: Curve) {
        self.input_curves[i] = curve;
    }
    fn set_output_curve(&mut self, i: usize, curve: Curve) {
        self.output_curves[i] = curve;
    }
    fn set_grid(&mut self, grid_8: Option<ByteView>, grid_16: Option<ByteView>) {
        self.grid_8 = grid_8;
        self.grid_16 = grid_16;
    }
}

impl MftTarget for B2A {
    // All as the A2B version above, except where noted.
    // Port of: modules/skcms/skcms.cc#L781-L806 (chrome/m156)
    fn read_mft_common(&mut self, buf: &[u8], tag_off: usize) -> bool {
        let b2a = self;
        // Same as A2B.
        b2a.matrix_channels = 0;
        b2a.input_channels = u32::from(byte_at(buf, tag_off + 8));
        b2a.output_channels = u32::from(byte_at(buf, tag_off + 9));

        // For B2A, exactly 3 input channels (XYZ) and 3 (RGB) or 4 (CMYK) output channels.
        if b2a.input_channels as usize != b2a.input_curves.len() {
            return false;
        }
        if b2a.output_channels < 3 || b2a.output_channels as usize > b2a.output_curves.len() {
            return false;
        }

        // Same as A2B.
        let mut total_grid_points: u64 = 1;
        for i in 0..b2a.input_channels as usize {
            b2a.grid_points[i] = byte_at(buf, tag_off + 10);
            total_grid_points *= u64::from(b2a.grid_points[i]);
        }
        if b2a.grid_points[0] < 2 || total_grid_points > SKCMS_MAX_GRID_POINTS {
            return false;
        }
        true
    }
    fn input_channels(&self) -> u32 {
        self.input_channels
    }
    fn output_channels(&self) -> u32 {
        self.output_channels
    }
    fn grid_points(&self) -> [u8; 4] {
        self.grid_points
    }
    fn set_input_curve(&mut self, i: usize, curve: Curve) {
        self.input_curves[i] = curve;
    }
    fn set_output_curve(&mut self, i: usize, curve: Curve) {
        self.output_curves[i] = curve;
    }
    fn set_grid(&mut self, grid_8: Option<ByteView>, grid_16: Option<ByteView>) {
        self.grid_8 = grid_8;
        self.grid_16 = grid_16;
    }
}

// Port of: modules/skcms/skcms.cc#L809-L861 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // mirrors the C++ casts of the ported arithmetic
fn init_tables<T: MftTarget>(
    shared: &Arc<[u8]>,
    table_base: usize,
    max_tables_len: u64,
    byte_width: u32,
    input_table_entries: u32,
    output_table_entries: u32,
    out: &mut T,
) -> bool {
    // byte_width is 1 or 2, [input|output]_table_entries are in [2, 4096], so no overflow
    let byte_len_per_input_table = input_table_entries * byte_width;
    let byte_len_per_output_table = output_table_entries * byte_width;

    // [input|output]_channels are <= 4, so still no overflow
    let byte_len_all_input_tables = out.input_channels() * byte_len_per_input_table;
    let byte_len_all_output_tables = out.output_channels() * byte_len_per_output_table;

    let mut grid_size = u64::from(out.output_channels() * byte_width);
    for axis in 0..out.input_channels() as usize {
        grid_size *= u64::from(out.grid_points()[axis]);
    }

    if max_tables_len
        < u64::from(byte_len_all_input_tables) + grid_size + u64::from(byte_len_all_output_tables)
    {
        return false;
    }

    let table_curve = |off: usize, entries: u32| -> Curve {
        let table = ByteView::new(Arc::clone(shared), off);
        if byte_width == 1 {
            Curve::Table8 { entries, table }
        } else {
            Curve::Table16 { entries, table }
        }
    };

    for i in 0..out.input_channels() {
        let off = table_base + (i * byte_len_per_input_table) as usize;
        out.set_input_curve(i as usize, table_curve(off, input_table_entries));
    }

    let grid_off = table_base + byte_len_all_input_tables as usize;
    if byte_width == 1 {
        out.set_grid(Some(ByteView::new(Arc::clone(shared), grid_off)), None);
    } else {
        out.set_grid(None, Some(ByteView::new(Arc::clone(shared), grid_off)));
    }

    let output_table_base = table_base + byte_len_all_input_tables as usize + grid_size as usize;
    for i in 0..out.output_channels() {
        let off = output_table_base + (i * byte_len_per_output_table) as usize;
        out.set_output_curve(i as usize, table_curve(off, output_table_entries));
    }

    true
}

// Port of: modules/skcms/skcms.cc#L864-L879 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // mirrors the C++ casts of the ported arithmetic
fn read_tag_mft1<T: MftTarget>(shared: &Arc<[u8]>, tag: &IccTag, out: &mut T) -> bool {
    if u64::from(tag.size) < MFT1_LAYOUT_FIXED_SIZE {
        return false;
    }

    if !out.read_mft_common(shared, tag.offset) {
        return false;
    }

    let input_table_entries = 256;
    let output_table_entries = 256;

    init_tables(
        shared,
        tag.offset + MFT1_LAYOUT_FIXED_SIZE as usize,
        u64::from(tag.size) - MFT1_LAYOUT_FIXED_SIZE,
        1,
        input_table_entries,
        output_table_entries,
        out,
    )
}

// Port of: modules/skcms/skcms.cc#L882-L903 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // mirrors the C++ casts of the ported arithmetic
fn read_tag_mft2<T: MftTarget>(shared: &Arc<[u8]>, tag: &IccTag, out: &mut T) -> bool {
    if u64::from(tag.size) < MFT2_LAYOUT_FIXED_SIZE {
        return false;
    }

    if !out.read_mft_common(shared, tag.offset) {
        return false;
    }

    let input_table_entries = u32::from(read_big_u16(shared, tag.offset + 48));
    let output_table_entries = u32::from(read_big_u16(shared, tag.offset + 50));

    // ICC spec mandates that 2 <= table_entries <= 4096
    if !(2..=4096).contains(&input_table_entries) || !(2..=4096).contains(&output_table_entries) {
        return false;
    }

    init_tables(
        shared,
        tag.offset + MFT2_LAYOUT_FIXED_SIZE as usize,
        u64::from(tag.size) - MFT2_LAYOUT_FIXED_SIZE,
        2,
        input_table_entries,
        output_table_entries,
        out,
    )
}

// Port of: modules/skcms/skcms.cc#L905-L930 (chrome/m156)
#[allow(clippy::needless_range_loop)] // mirrors the C++ index loops
fn read_curves(
    shared: &Arc<[u8]>,
    tag_off: usize,
    size: u32,
    mut curve_offset: u32,
    num_curves: u32,
    curves: &mut [Curve],
) -> bool {
    for i in 0..num_curves as usize {
        if curve_offset > size {
            return false;
        }

        let Some((curve, mut curve_bytes)) =
            read_curve(shared, tag_off + curve_offset as usize, size - curve_offset)
        else {
            return false;
        };
        curves[i] = curve;

        if curve_bytes > u32::MAX - 3 {
            return false;
        }
        curve_bytes = (curve_bytes + 3) & !3u32;

        let new_offset_64 = u64::from(curve_offset) + u64::from(curve_bytes);
        let Ok(new_offset) = u32::try_from(new_offset_64) else {
            return false;
        };
        curve_offset = new_offset;
    }

    true
}

// Port of: modules/skcms/skcms.cc#L953-L1095 (chrome/m156)
#[allow(clippy::too_many_lines)] // a single tag reader, as in C++
#[allow(clippy::cast_possible_truncation)] // mirrors the C++ casts of the ported arithmetic
#[allow(clippy::if_not_else)] // mirrors the C++ branch order
fn read_tag_mab(shared: &Arc<[u8]>, tag: &IccTag, a2b: &mut A2B, pcs_is_xyz: bool) -> bool {
    if u64::from(tag.size) < MAB_LAYOUT_SIZE {
        return false;
    }

    let t = tag.offset;
    a2b.input_channels = u32::from(byte_at(shared, t + 8));
    a2b.output_channels = u32::from(byte_at(shared, t + 9));

    // We require exactly three (ie XYZ/Lab/RGB) output channels
    if a2b.output_channels as usize != a2b.output_curves.len() {
        return false;
    }
    // We require no more than four (ie CMYK) input channels
    if a2b.input_channels as usize > a2b.input_curves.len() {
        return false;
    }

    let b_curve_offset = read_big_u32(shared, t + 12);
    let matrix_offset = read_big_u32(shared, t + 16);
    let m_curve_offset = read_big_u32(shared, t + 20);
    let clut_offset = read_big_u32(shared, t + 24);
    let a_curve_offset = read_big_u32(shared, t + 28);

    // "B" curves must be present
    if 0 == b_curve_offset {
        return false;
    }

    if !read_curves(
        shared,
        t,
        tag.size,
        b_curve_offset,
        a2b.output_channels,
        &mut a2b.output_curves,
    ) {
        return false;
    }

    // "M" curves and Matrix must be used together
    if 0 != m_curve_offset {
        if 0 == matrix_offset {
            return false;
        }
        a2b.matrix_channels = a2b.output_channels;
        if !read_curves(
            shared,
            t,
            tag.size,
            m_curve_offset,
            a2b.matrix_channels,
            &mut a2b.matrix_curves,
        ) {
            return false;
        }

        // Read matrix, which is stored as a row-major 3x3, followed by the fourth column
        if u64::from(tag.size) < u64::from(matrix_offset) + 12 * 4 {
            return false;
        }
        let encoding_factor = if pcs_is_xyz {
            65535.0f32 / 32768.0f32
        } else {
            1.0f32
        };
        let mtx = t + matrix_offset as usize;
        a2b.matrix = read_matrix3x4(shared, mtx, encoding_factor);
    } else {
        if 0 != matrix_offset {
            return false;
        }
        a2b.matrix_channels = 0;
    }

    // "A" curves and CLUT must be used together
    if 0 != a_curve_offset {
        if 0 == clut_offset {
            return false;
        }
        if !read_curves(
            shared,
            t,
            tag.size,
            a_curve_offset,
            a2b.input_channels,
            &mut a2b.input_curves,
        ) {
            return false;
        }

        if u64::from(tag.size) < u64::from(clut_offset) + CLUT_LAYOUT_FIXED_SIZE {
            return false;
        }
        let clut = t + clut_offset as usize;
        let grid_byte_width = byte_at(shared, clut + 16);
        let variable = clut + CLUT_LAYOUT_FIXED_SIZE as usize;

        if grid_byte_width == 1 {
            a2b.grid_8 = Some(ByteView::new(Arc::clone(shared), variable));
            a2b.grid_16 = None;
        } else if grid_byte_width == 2 {
            a2b.grid_8 = None;
            a2b.grid_16 = Some(ByteView::new(Arc::clone(shared), variable));
        } else {
            return false;
        }

        // the payload
        let mut grid_size = u64::from(a2b.output_channels * u32::from(grid_byte_width));
        let mut total_grid_points: u64 = 1;
        for i in 0..a2b.input_channels as usize {
            a2b.grid_points[i] = byte_at(shared, clut + i);
            // The grid only makes sense with at least two points along each axis
            if a2b.grid_points[i] < 2 {
                return false;
            }
            grid_size *= u64::from(a2b.grid_points[i]);
            total_grid_points *= u64::from(a2b.grid_points[i]);
        }

        if total_grid_points > SKCMS_MAX_GRID_POINTS {
            return false;
        }

        let table_size = u64::from(clut_offset) + CLUT_LAYOUT_FIXED_SIZE + grid_size;
        if table_size > u64::from(tag.size) {
            return false;
        }

        // gather_24 and gather_48 read 1 or 2 extra bytes.
        // We must ensure that those extra bytes are within the provided buffer limit.
        let mut slack = 0u64;
        if a2b.output_channels == 3 {
            slack = if grid_byte_width == 1 { 1 } else { 2 };
        }
        if t as u64 + table_size + slack > shared.len() as u64 {
            return false;
        }
    } else {
        if 0 != clut_offset {
            return false;
        }

        // If there is no CLUT, the number of input and output channels must match
        if a2b.input_channels != a2b.output_channels {
            return false;
        }

        // Zero out the number of input channels to signal that we're skipping this stage
        a2b.input_channels = 0;
    }

    true
}

// Reads a 3x4 matrix, stored as a row-major 3x3 followed by the fourth column, each entry scaled
// by `encoding_factor`.
fn read_matrix3x4(buf: &[u8], mtx: usize, encoding_factor: f32) -> Matrix3x4 {
    let mut m = Matrix3x4::default();
    m.vals[0][0] = encoding_factor * read_big_fixed(buf, mtx);
    m.vals[0][1] = encoding_factor * read_big_fixed(buf, mtx + 4);
    m.vals[0][2] = encoding_factor * read_big_fixed(buf, mtx + 8);
    m.vals[1][0] = encoding_factor * read_big_fixed(buf, mtx + 12);
    m.vals[1][1] = encoding_factor * read_big_fixed(buf, mtx + 16);
    m.vals[1][2] = encoding_factor * read_big_fixed(buf, mtx + 20);
    m.vals[2][0] = encoding_factor * read_big_fixed(buf, mtx + 24);
    m.vals[2][1] = encoding_factor * read_big_fixed(buf, mtx + 28);
    m.vals[2][2] = encoding_factor * read_big_fixed(buf, mtx + 32);
    m.vals[0][3] = encoding_factor * read_big_fixed(buf, mtx + 36);
    m.vals[1][3] = encoding_factor * read_big_fixed(buf, mtx + 40);
    m.vals[2][3] = encoding_factor * read_big_fixed(buf, mtx + 44);
    m
}

// Exactly the same as read_tag_mab(), except where there are comments.
// TODO: refactor the two to eliminate common code?
// Port of: modules/skcms/skcms.cc#L1099-L1237 (chrome/m156)
#[allow(clippy::too_many_lines)] // a single tag reader, as in C++
#[allow(clippy::cast_possible_truncation)] // mirrors the C++ casts of the ported arithmetic
#[allow(clippy::if_not_else)] // mirrors the C++ branch order
fn read_tag_mba(shared: &Arc<[u8]>, tag: &IccTag, b2a: &mut B2A, pcs_is_xyz: bool) -> bool {
    if u64::from(tag.size) < MAB_LAYOUT_SIZE {
        return false;
    }

    let t = tag.offset;
    b2a.input_channels = u32::from(byte_at(shared, t + 8));
    b2a.output_channels = u32::from(byte_at(shared, t + 9));

    // Require exactly 3 inputs (XYZ) and 3 (RGB) or 4 (CMYK) outputs.
    if b2a.input_channels as usize != b2a.input_curves.len() {
        return false;
    }
    if b2a.output_channels < 3 || b2a.output_channels as usize > b2a.output_curves.len() {
        return false;
    }

    let b_curve_offset = read_big_u32(shared, t + 12);
    let matrix_offset = read_big_u32(shared, t + 16);
    let m_curve_offset = read_big_u32(shared, t + 20);
    let clut_offset = read_big_u32(shared, t + 24);
    let a_curve_offset = read_big_u32(shared, t + 28);

    if 0 == b_curve_offset {
        return false;
    }

    // "B" curves are our inputs, not outputs.
    if !read_curves(
        shared,
        t,
        tag.size,
        b_curve_offset,
        b2a.input_channels,
        &mut b2a.input_curves,
    ) {
        return false;
    }

    if 0 != m_curve_offset {
        if 0 == matrix_offset {
            return false;
        }
        // Matrix channels is tied to input_channels (3), not output_channels.
        b2a.matrix_channels = b2a.input_channels;

        if !read_curves(
            shared,
            t,
            tag.size,
            m_curve_offset,
            b2a.matrix_channels,
            &mut b2a.matrix_curves,
        ) {
            return false;
        }

        if u64::from(tag.size) < u64::from(matrix_offset) + 12 * 4 {
            return false;
        }
        let encoding_factor = if pcs_is_xyz {
            32768.0f32 / 65535.0f32
        } else {
            1.0f32
        }; // TODO: understand
        let mtx = t + matrix_offset as usize;
        b2a.matrix = read_matrix3x4(shared, mtx, encoding_factor);
    } else {
        if 0 != matrix_offset {
            return false;
        }
        b2a.matrix_channels = 0;
    }

    if 0 != a_curve_offset {
        if 0 == clut_offset {
            return false;
        }

        // "A" curves are our output, not input.
        if !read_curves(
            shared,
            t,
            tag.size,
            a_curve_offset,
            b2a.output_channels,
            &mut b2a.output_curves,
        ) {
            return false;
        }

        if u64::from(tag.size) < u64::from(clut_offset) + CLUT_LAYOUT_FIXED_SIZE {
            return false;
        }
        let clut = t + clut_offset as usize;
        let grid_byte_width = byte_at(shared, clut + 16);
        let variable = clut + CLUT_LAYOUT_FIXED_SIZE as usize;

        if grid_byte_width == 1 {
            b2a.grid_8 = Some(ByteView::new(Arc::clone(shared), variable));
            b2a.grid_16 = None;
        } else if grid_byte_width == 2 {
            b2a.grid_8 = None;
            b2a.grid_16 = Some(ByteView::new(Arc::clone(shared), variable));
        } else {
            return false;
        }

        let mut grid_size = u64::from(b2a.output_channels * u32::from(grid_byte_width));
        let mut total_grid_points: u64 = 1;
        for i in 0..b2a.input_channels as usize {
            b2a.grid_points[i] = byte_at(shared, clut + i);
            if b2a.grid_points[i] < 2 {
                return false;
            }
            grid_size *= u64::from(b2a.grid_points[i]);
            total_grid_points *= u64::from(b2a.grid_points[i]);
        }

        if total_grid_points > SKCMS_MAX_GRID_POINTS {
            return false;
        }

        if u64::from(tag.size) < u64::from(clut_offset) + CLUT_LAYOUT_FIXED_SIZE + grid_size {
            return false;
        }

        // gather_24 and gather_48 read 1 or 2 extra bytes.
        // We must ensure that those extra bytes are within the provided buffer limit.
        let mut slack = 0u64;
        if b2a.output_channels == 3 {
            slack = if grid_byte_width == 1 { 1 } else { 2 };
        }
        if t as u64 + u64::from(clut_offset) + CLUT_LAYOUT_FIXED_SIZE + grid_size + slack
            > shared.len() as u64
        {
            return false;
        }
    } else {
        if 0 != clut_offset {
            return false;
        }

        if b2a.input_channels != b2a.output_channels {
            return false;
        }

        // Zero out *output* channels to skip this stage.
        b2a.output_channels = 0;
    }
    true
}

// Port of: modules/skcms/skcms.cc#L1293-L1298 (chrome/m156)
fn read_a2b(shared: &Arc<[u8]>, tag: &IccTag, a2b: &mut A2B, pcs_is_xyz: bool) -> bool {
    if tag.type_ == sig::MFT1 {
        return read_tag_mft1(shared, tag, a2b);
    }
    if tag.type_ == sig::MFT2 {
        return read_tag_mft2(shared, tag, a2b);
    }
    if tag.type_ == sig::MAB {
        return read_tag_mab(shared, tag, a2b, pcs_is_xyz);
    }
    false
}

// Port of: modules/skcms/skcms.cc#L1300-L1305 (chrome/m156)
fn read_b2a(shared: &Arc<[u8]>, tag: &IccTag, b2a: &mut B2A, pcs_is_xyz: bool) -> bool {
    if tag.type_ == sig::MFT1 {
        return read_tag_mft1(shared, tag, b2a);
    }
    if tag.type_ == sig::MFT2 {
        return read_tag_mft2(shared, tag, b2a);
    }
    if tag.type_ == sig::MBA {
        return read_tag_mba(shared, tag, b2a, pcs_is_xyz);
    }
    false
}

// Port of: modules/skcms/skcms.cc#L1316-L1328 (chrome/m156)
fn read_cicp(buf: &[u8], tag: &IccTag) -> Option<Cicp> {
    if tag.type_ != sig::CICP || u64::from(tag.size) < CICP_LAYOUT_SIZE {
        return None;
    }

    Some(Cicp {
        color_primaries: byte_at(buf, tag.offset + 8),
        transfer_characteristics: byte_at(buf, tag.offset + 9),
        matrix_coefficients: byte_at(buf, tag.offset + 10),
        video_full_range_flag: byte_at(buf, tag.offset + 11),
    })
}

// Port of: modules/skcms/skcms.cc#L1336-L1350 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // mirrors the C++ casts of the ported arithmetic
fn read_hagc(shared: &Arc<[u8]>, tag: &IccTag) -> Option<Hagc> {
    if tag.type_ != sig::HAGC_TYPE || u64::from(tag.size) < HAGC_LAYOUT_SIZE {
        return None;
    }

    let size = read_big_u32(shared, tag.offset + 8);
    if u64::from(size) > u64::from(tag.size) - HAGC_LAYOUT_SIZE {
        return None;
    }

    Some(Hagc {
        size,
        buffer: Some(ByteView::new(
            Arc::clone(shared),
            tag.offset + HAGC_LAYOUT_SIZE as usize,
        )),
    })
}

// Port of: modules/skcms/skcms.cc#L1377-L1380 (chrome/m156)
fn usable_as_src(profile: &IccProfile) -> bool {
    profile.has_a2b || (profile.has_trc && profile.has_to_xyzd50)
}

/// Parses an ICC profile and returns it if possible.
/// Selects an A2B profile (if present) according to the priority list (each entry 0-2).
/// The buffer is copied into the profile (skcms borrows it instead).
/// The parse will fail if there is not enough padding (typically 1-2 bytes) past the end of the
/// profile for internally optimized read calls.
// Port of: modules/skcms/skcms.cc#L1382-L1541 (chrome/m156)
#[doc(alias = "skcms_ParseWithA2BPriority")]
#[must_use]
#[allow(clippy::too_many_lines)] // one function, as in C++
#[allow(clippy::cast_possible_truncation)] // mirrors the C++ casts of the ported arithmetic
#[allow(clippy::cast_sign_loss)] // mirrors the C++ casts of the ported arithmetic
pub fn parse_with_a2b_priority(buf: &[u8], priority: &[i32]) -> Option<IccProfile> {
    let len = buf.len();
    let mut profile = IccProfile::default();

    if (len as u64) < HEADER_LAYOUT_SIZE {
        return None;
    }

    // Byte-swap all header fields
    let shared: Arc<[u8]> = Arc::from(buf);
    profile.buffer = Some(Arc::clone(&shared));
    profile.size = read_big_u32(buf, 0);
    let version = read_big_u32(buf, 8);
    profile.data_color_space = read_big_u32(buf, 16);
    profile.pcs = read_big_u32(buf, 20);
    let file_signature = read_big_u32(buf, 36);
    let illuminant_x = read_big_fixed(buf, 68);
    let illuminant_y = read_big_fixed(buf, 72);
    let illuminant_z = read_big_fixed(buf, 76);
    profile.tag_count = read_big_u32(buf, 128);

    // Validate signature, size (smaller than buffer, large enough to hold tag table),
    // and major version
    let tag_table_size = u64::from(profile.tag_count) * TAG_LAYOUT_SIZE;
    if file_signature != sig::ACSP
        || profile.size as usize > len
        || u64::from(profile.size) < HEADER_LAYOUT_SIZE + tag_table_size
        || (version >> 24) > 4
    {
        return None;
    }

    // Validate that illuminant is D50 white
    if fabsf_(illuminant_x - 0.9642f32) > 0.0100f32
        || fabsf_(illuminant_y - 1.0000f32) > 0.0100f32
        || fabsf_(illuminant_z - 0.8249f32) > 0.0100f32
    {
        return None;
    }

    // Validate that all tag entries have sane offset + size
    for i in 0..profile.tag_count as usize {
        let entry = HEADER_LAYOUT_SIZE as usize + i * TAG_LAYOUT_SIZE as usize;
        let tag_offset = read_big_u32(buf, entry + 4);
        let tag_size = read_big_u32(buf, entry + 8);
        let tag_end = u64::from(tag_offset) + u64::from(tag_size);
        if tag_size < 4 || tag_end > u64::from(profile.size) {
            return None;
        }
    }

    if profile.pcs != signature::XYZ && profile.pcs != signature::LAB {
        return None;
    }

    let pcs_is_xyz = profile.pcs == signature::XYZ;

    // Pre-parse commonly used tags.
    let k_trc = if profile.data_color_space == signature::GRAY {
        get_tag_by_signature(&profile, sig::K_TRC)
    } else {
        None
    };
    if let Some(k_trc) = k_trc {
        let Some((curve, _)) = read_curve(&shared, k_trc.offset, k_trc.size) else {
            // Malformed tag
            return None;
        };
        profile.trc[1] = curve.clone();
        profile.trc[2] = curve.clone();
        profile.trc[0] = curve;
        profile.has_trc = true;

        if pcs_is_xyz {
            profile.to_xyzd50.vals[0][0] = illuminant_x;
            profile.to_xyzd50.vals[1][1] = illuminant_y;
            profile.to_xyzd50.vals[2][2] = illuminant_z;
            profile.has_to_xyzd50 = true;
        }
    } else {
        if let (Some(r_trc), Some(g_trc), Some(b_trc)) = (
            get_tag_by_signature(&profile, sig::R_TRC),
            get_tag_by_signature(&profile, sig::G_TRC),
            get_tag_by_signature(&profile, sig::B_TRC),
        ) {
            let (Some((r, _)), Some((g, _)), Some((b, _))) = (
                read_curve(&shared, r_trc.offset, r_trc.size),
                read_curve(&shared, g_trc.offset, g_trc.size),
                read_curve(&shared, b_trc.offset, b_trc.size),
            ) else {
                // Malformed TRC tags
                return None;
            };
            profile.trc = [r, g, b];
            profile.has_trc = true;
        }

        if let (Some(r_xyz), Some(g_xyz), Some(b_xyz)) = (
            get_tag_by_signature(&profile, sig::R_XYZ),
            get_tag_by_signature(&profile, sig::G_XYZ),
            get_tag_by_signature(&profile, sig::B_XYZ),
        ) {
            let Some(m) = read_to_xyzd50(buf, &r_xyz, &g_xyz, &b_xyz) else {
                // Malformed XYZ tags
                return None;
            };
            profile.to_xyzd50 = m;
            profile.has_to_xyzd50 = true;
        }
    }

    for &p in priority {
        // enum { perceptual, relative_colormetric, saturation }
        if !(0..=2).contains(&p) {
            return None;
        }
        let tag_sig = sig::A2B0 + p as u32;
        if let Some(tag) = get_tag_by_signature(&profile, tag_sig) {
            if !read_a2b(&shared, &tag, &mut profile.a2b, pcs_is_xyz) {
                // Malformed A2B tag
                return None;
            }
            profile.has_a2b = true;
            break;
        }
    }

    for &p in priority {
        // enum { perceptual, relative_colormetric, saturation }
        if !(0..=2).contains(&p) {
            return None;
        }
        let tag_sig = sig::B2A0 + p as u32;
        if let Some(tag) = get_tag_by_signature(&profile, tag_sig) {
            if !read_b2a(&shared, &tag, &mut profile.b2a, pcs_is_xyz) {
                // Malformed B2A tag
                return None;
            }
            profile.has_b2a = true;
            break;
        }
    }

    if let Some(cicp_tag) = get_tag_by_signature(&profile, sig::CICP) {
        let Some(cicp) = read_cicp(buf, &cicp_tag) else {
            // Malformed CICP tag
            return None;
        };
        profile.cicp = cicp;
        profile.has_cicp = true;
    }

    if let Some(hagc_tag) = get_tag_by_signature(&profile, sig::HAGC) {
        let Some(hagc) = read_hagc(&shared, &hagc_tag) else {
            // Malformed HAGC tag
            return None;
        };
        profile.hagc = hagc;
        profile.has_hagc = true;
    }

    if usable_as_src(&profile) {
        Some(profile)
    } else {
        None
    }
}

/// Parses an ICC profile. For continuity of existing user expectations, prefers A2B0
/// (perceptual) over A2B1 (relative colormetric), and ignores A2B2 (saturation).
// Port of: modules/skcms/src/skcms_public.h#L315-L322 (chrome/m156)
#[doc(alias = "skcms_Parse")]
#[must_use]
pub fn parse(buf: &[u8]) -> Option<IccProfile> {
    parse_with_a2b_priority(buf, &[0, 1])
}

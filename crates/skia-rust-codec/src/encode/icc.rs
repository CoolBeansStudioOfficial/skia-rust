// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/encode/SkICC.cpp (chrome/m156), the parts that write an ICC profile for a colour
// space whose transfer function is sRGB-like (the SDR profiles PNG encoding needs), and
// src/encode/SkICCPriv.h. The HDR tables (PQ, HLG: the `mAB`/`mBA` tags, the CLUT and the `cicp`
// and `HAGC` tags) are not ported: `write_icc_profile` returns `None` for those colour spaces, so
// a PNG of them gets no `iCCP` chunk.

// Clippy: a line-by-line port of Skia's C++, whose float and integer conversions are kept as
// written so they can be compared with the C.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::float_cmp
)]

use skia_rust_core::color_space::{ColorSpace, named_gamut, named_transfer_fn};
use skia_rust_core::md5::Md5;
use skia_rust_core::stream::{DynamicMemoryWStream, WStream};
use skia_rust_skcms::{Curve, IccProfile, Matrix3x3, TfType, TransferFunction};

/// Port of `kD50_x`, `kD50_y`, `kD50_z` (SkICC.cpp#L40-L42).
const KD50_X: f32 = 0.9642;
const KD50_Y: f32 = 1.0000;
const KD50_Z: f32 = 0.8249;

/// Port of `kICCHeaderSize` (SkICCPriv.h): the header, including the tag count.
const ICC_HEADER_SIZE: usize = 132;
/// Port of `kICCTagTableEntrySize` (SkICCPriv.h).
const ICC_TAG_TABLE_ENTRY_SIZE: usize = 12;

/// Port of the four-byte tags of SkICCPriv.h (`SkSetFourByteTag`).
const fn tag(a: u8, b: u8, c: u8, d: u8) -> u32 {
    ((a as u32) << 24) | ((b as u32) << 16) | ((c as u32) << 8) | (d as u32)
}
const RGB_COLOR_SPACE: u32 = tag(b'R', b'G', b'B', b' ');
const DISPLAY_PROFILE: u32 = tag(b'm', b'n', b't', b'r');
const XYZ_PCS_SPACE: u32 = tag(b'X', b'Y', b'Z', b' ');
const ACSP_SIGNATURE: u32 = tag(b'a', b'c', b's', b'p');
const TAG_RXYZ: u32 = tag(b'r', b'X', b'Y', b'Z');
const TAG_GXYZ: u32 = tag(b'g', b'X', b'Y', b'Z');
const TAG_BXYZ: u32 = tag(b'b', b'X', b'Y', b'Z');
const TAG_RTRC: u32 = tag(b'r', b'T', b'R', b'C');
const TAG_GTRC: u32 = tag(b'g', b'T', b'R', b'C');
const TAG_BTRC: u32 = tag(b'b', b'T', b'R', b'C');
const TAG_DESC: u32 = tag(b'd', b'e', b's', b'c');
const TAG_WTPT: u32 = tag(b'w', b't', b'p', b't');
const TAG_CPRT: u32 = tag(b'c', b'p', b'r', b't');
const TAG_PARA_CURVE_TYPE: u32 = tag(b'p', b'a', b'r', b'a');
const TAG_TEXT_TYPE: u32 = tag(b'm', b'l', b'u', b'c');
/// Port of `kGABCDEF_ParaCurveType` and `kExponential_ParaCurveType` (SkICCPriv.h).
const GABCDEF_PARA_CURVE_TYPE: u16 = 4;
const EXPONENTIAL_PARA_CURVE_TYPE: u16 = 0;

/// Port of `kCICPPrimaries*` (SkICC.cpp#L193-L195).
const CICP_PRIMARIES_SRGB: u32 = 1;
const CICP_PRIMARIES_P3: u32 = 12;
const CICP_PRIMARIES_REC2020: u32 = 9;
/// Port of `kCICPTrfn*` (SkICC.cpp#L208-L212).
const CICP_TRFN_SRGB: u32 = 1;
const CICP_TRFN_2DOT2: u32 = 4;
const CICP_TRFN_LINEAR: u32 = 8;
const CICP_TRFN_PQ: u32 = 16;
const CICP_TRFN_HLG: u32 = 18;

/// Port of `SK_MaxS32FitsInFloat` (SkFloatingPoint.h#L68).
const SK_MAX_S32_FITS_IN_FLOAT: f32 = 2_147_483_520.0;

/// Port of `float_round_to_fixed` (SkICC.cpp#L48-L51): `sk_float_saturate2int` of the
/// double-precision product, rounded to the nearest 16.16 value.
fn float_round_to_fixed(x: f32) -> i32 {
    let v = (f64::from(x) * 65536.0 + 0.5).floor() as f32;
    let v = if v < SK_MAX_S32_FITS_IN_FLOAT {
        v
    } else {
        SK_MAX_S32_FITS_IN_FLOAT
    };
    let v = if v > -SK_MAX_S32_FITS_IN_FLOAT {
        v
    } else {
        -SK_MAX_S32_FITS_IN_FLOAT
    };
    v as i32
}

/// Port of `write_xyz_tag` (SkICC.cpp#L133-L142).
fn write_xyz_tag(x: f32, y: f32, z: f32) -> Vec<u8> {
    let mut out = Vec::with_capacity(20);
    out.extend_from_slice(&XYZ_PCS_SPACE.to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(&float_round_to_fixed(x).to_be_bytes());
    out.extend_from_slice(&float_round_to_fixed(y).to_be_bytes());
    out.extend_from_slice(&float_round_to_fixed(z).to_be_bytes());
    out
}

/// Port of `nearly_equal(float, float)` (SkICC.cpp#L159-L163).
fn nearly_equal(x: f32, y: f32) -> bool {
    const TOLERANCE: f32 = 1.0 / (1 << 11) as f32;
    (x - y).abs() <= TOLERANCE
}

/// Port of `nearly_equal(skcms_TransferFunction, skcms_TransferFunction)` (SkICC.cpp#L171-L180).
fn nearly_equal_tf(u: &TransferFunction, v: &TransferFunction) -> bool {
    nearly_equal(u.g, v.g)
        && nearly_equal(u.a, v.a)
        && nearly_equal(u.b, v.b)
        && nearly_equal(u.c, v.c)
        && nearly_equal(u.d, v.d)
        && nearly_equal(u.e, v.e)
        && nearly_equal(u.f, v.f)
}

/// Port of `nearly_equal(skcms_Matrix3x3, skcms_Matrix3x3)` (SkICC.cpp#L182-L195).
fn nearly_equal_matrix(u: &Matrix3x3, v: &Matrix3x3) -> bool {
    for r in 0..3 {
        for c in 0..3 {
            if !nearly_equal(u.vals[r][c], v.vals[r][c]) {
                return false;
            }
        }
    }
    true
}

/// Port of `get_cicp_primaries` (SkICC.cpp#L197-L210).
fn get_cicp_primaries(to_xyzd50: &Matrix3x3) -> u32 {
    if nearly_equal_matrix(to_xyzd50, &named_gamut::SRGB) {
        CICP_PRIMARIES_SRGB
    } else if nearly_equal_matrix(to_xyzd50, &named_gamut::DISPLAY_P3) {
        CICP_PRIMARIES_P3
    } else if nearly_equal_matrix(to_xyzd50, &named_gamut::REC2020) {
        CICP_PRIMARIES_REC2020
    } else {
        0
    }
}

/// Port of `get_cicp_trfn` (SkICC.cpp#L214-L240).
fn get_cicp_trfn(fn_: &TransferFunction) -> u32 {
    match fn_.tf_type() {
        TfType::SRGBish => {
            if nearly_equal_tf(fn_, &named_transfer_fn::SRGB) {
                CICP_TRFN_SRGB
            } else if nearly_equal_tf(fn_, &named_transfer_fn::DOT22) {
                CICP_TRFN_2DOT2
            } else if nearly_equal_tf(fn_, &named_transfer_fn::LINEAR) {
                CICP_TRFN_LINEAR
            } else {
                0
            }
        }
        TfType::PQ | TfType::PQish => CICP_TRFN_PQ,
        TfType::HLG | TfType::HLGish => CICP_TRFN_HLG,
        _ => 0,
    }
}

/// Port of `get_desc_string` (SkICC.cpp#L242-L300).
fn get_desc_string(fn_: &TransferFunction, to_xyzd50: &Matrix3x3) -> String {
    let cicp_trfn = get_cicp_trfn(fn_);
    let cicp_primaries = get_cicp_primaries(to_xyzd50);
    if cicp_trfn == CICP_PRIMARIES_SRGB && cicp_primaries == CICP_TRFN_SRGB {
        return "sRGB".to_owned();
    }
    if cicp_primaries != 0 && cicp_trfn != 0 {
        let mut result = String::new();
        result.push_str(match cicp_primaries {
            CICP_PRIMARIES_SRGB => "sRGB",
            CICP_PRIMARIES_P3 => "Display P3",
            CICP_PRIMARIES_REC2020 => "Rec2020",
            _ => "Unknown",
        });
        result.push_str(" Gamut with ");
        result.push_str(match cicp_trfn {
            CICP_TRFN_SRGB => "sRGB",
            CICP_TRFN_LINEAR => "Linear",
            CICP_TRFN_2DOT2 => "2.2",
            CICP_TRFN_PQ => "PQ",
            CICP_TRFN_HLG => "HLG",
            _ => "Unknown",
        });
        result.push_str(" Transfer");
        return result;
    }
    let mut md5 = Md5::new();
    for v in [to_xyzd50.vals[0], to_xyzd50.vals[1], to_xyzd50.vals[2]] {
        for x in v {
            md5.write_bytes(&x.to_ne_bytes());
        }
    }
    for x in [fn_.g, fn_.a, fn_.b, fn_.c, fn_.d, fn_.e, fn_.f] {
        md5.write_bytes(&x.to_ne_bytes());
    }
    let digest = md5.finish();
    format!("Google/Skia/{}", digest.to_hex_string())
}

/// Port of `write_text_tag` (SkICC.cpp#L302-L323): a `mluc` tag with one English (US) record.
fn write_text_tag(text: &str) -> Vec<u8> {
    let text_length = text.len() as u32;
    let mut s = DynamicMemoryWStream::new();
    let header: [u32; 7] = [
        TAG_TEXT_TYPE,               // Type signature
        0,                           // Reserved
        1,                           // Number of records
        12,                          // Record size (must be 12)
        tag(b'e', b'n', b'U', b'S'), // English USA
        2 * text_length,             // Length of string in bytes
        28,                          // Offset of string
    ];
    for h in header {
        s.write(&h.to_be_bytes());
    }
    for &b in text.as_bytes() {
        s.write(&[0, b]);
    }
    s.pad_to_align4();
    s.detach_as_vector()
}

/// Port of `write_trc_tag` (SkICC.cpp#L398-L432) for the parametric curve that the sRGB-like
/// transfer functions use (`table_entries` is always zero here).
fn write_trc_tag(fn_: &TransferFunction) -> Vec<u8> {
    let mut s = DynamicMemoryWStream::new();
    s.write(&TAG_PARA_CURVE_TYPE.to_be_bytes());
    s.write(&0u32.to_be_bytes());
    if fn_.a == 1.0 && fn_.b == 0.0 && fn_.c == 0.0 && fn_.d == 0.0 && fn_.e == 0.0 && fn_.f == 0.0
    {
        s.write(&EXPONENTIAL_PARA_CURVE_TYPE.to_be_bytes());
        s.write(&0u16.to_be_bytes());
        s.write(&float_round_to_fixed(fn_.g).to_be_bytes());
    } else {
        s.write(&GABCDEF_PARA_CURVE_TYPE.to_be_bytes());
        s.write(&0u16.to_be_bytes());
        for x in [fn_.g, fn_.a, fn_.b, fn_.c, fn_.d, fn_.e, fn_.f] {
            s.write(&float_round_to_fixed(x).to_be_bytes());
        }
    }
    s.pad_to_align4();
    s.detach_as_vector()
}

/// The parts of `skcms_ICCProfile` that the colour-space path fills in (SkICC.cpp#L709-L870):
/// an RGB display profile with a matrix to XYZ D50, and one sRGB-like curve for all channels.
struct Profile {
    to_xyzd50: Matrix3x3,
    trc: Option<TransferFunction>,
}

/// Port of `SkWriteICCProfile(const skcms_ICCProfile*, const char* desc)` (SkICC.cpp#L564-L707)
/// for the tags the profile above uses: `desc`, the colorants, `wtpt`, the curves and `cprt`.
fn write_profile(profile: &Profile, desc: &str) -> Vec<u8> {
    let m = &profile.to_xyzd50;
    // (tag, data). An empty vector is a tag that shares the data of the previous curve.
    let mut tags: Vec<(u32, Vec<u8>)> = vec![
        (
            TAG_RXYZ,
            write_xyz_tag(m.vals[0][0], m.vals[1][0], m.vals[2][0]),
        ),
        (
            TAG_GXYZ,
            write_xyz_tag(m.vals[0][1], m.vals[1][1], m.vals[2][1]),
        ),
        (
            TAG_BXYZ,
            write_xyz_tag(m.vals[0][2], m.vals[1][2], m.vals[2][2]),
        ),
        (TAG_WTPT, write_xyz_tag(KD50_X, KD50_Y, KD50_Z)),
    ];
    if let Some(trc) = &profile.trc {
        // The three curves are equal in this path, so the green and blue tags are empty.
        tags.push((TAG_RTRC, write_trc_tag(trc)));
        tags.push((TAG_GTRC, Vec::new()));
        tags.push((TAG_BTRC, Vec::new()));
    }
    tags.push((TAG_CPRT, write_text_tag("Google Inc. 2016")));
    tags.insert(0, (TAG_DESC, write_text_tag(desc)));

    let tag_data_size: usize = tags.iter().map(|(_, d)| d.len()).sum();
    let tag_table_size = ICC_TAG_TABLE_ENTRY_SIZE * tags.len();
    let profile_size = ICC_HEADER_SIZE + tag_table_size + tag_data_size;

    let mut out: Vec<u8> = Vec::with_capacity(profile_size);
    // ICCHeader (SkICC.cpp#L67-L97). The version is 4.3: no CICP or HAGC tags are written.
    out.extend_from_slice(&(profile_size as u32).to_be_bytes()); // size
    out.extend_from_slice(&0u32.to_be_bytes()); // cmm_type
    out.extend_from_slice(&0x0430_0000u32.to_be_bytes()); // version
    out.extend_from_slice(&DISPLAY_PROFILE.to_be_bytes()); // profile_class
    out.extend_from_slice(&RGB_COLOR_SPACE.to_be_bytes()); // data_color_space
    out.extend_from_slice(&XYZ_PCS_SPACE.to_be_bytes()); // pcs
    out.extend_from_slice(&2016u16.to_be_bytes()); // creation_date_year
    out.extend_from_slice(&1u16.to_be_bytes()); // creation_date_month
    out.extend_from_slice(&1u16.to_be_bytes()); // creation_date_day
    out.extend_from_slice(&[0u8; 6]); // hours, minutes, seconds
    out.extend_from_slice(&ACSP_SIGNATURE.to_be_bytes()); // signature
    out.extend_from_slice(&[0u8; 4]); // platform
    out.extend_from_slice(&[0u8; 4]); // flags
    out.extend_from_slice(&[0u8; 4]); // device_manufacturer
    out.extend_from_slice(&[0u8; 4]); // device_model
    out.extend_from_slice(&[0u8; 8]); // device_attributes
    out.extend_from_slice(&1u32.to_be_bytes()); // rendering_intent
    out.extend_from_slice(&float_round_to_fixed(KD50_X).to_be_bytes());
    out.extend_from_slice(&float_round_to_fixed(KD50_Y).to_be_bytes());
    out.extend_from_slice(&float_round_to_fixed(KD50_Z).to_be_bytes());
    out.extend_from_slice(&[0u8; 4]); // creator
    out.extend_from_slice(&[0u8; 16]); // profile_id
    out.extend_from_slice(&[0u8; 28]); // reserved
    out.extend_from_slice(&(tags.len() as u32).to_be_bytes()); // tag_count
    debug_assert_eq!(out.len(), ICC_HEADER_SIZE);

    // The tag table. An empty tag takes the offset and size of the last non-empty one, as in C.
    let mut last_tag_offset = ICC_HEADER_SIZE + tag_table_size;
    let mut last_tag_size = 0usize;
    for (name, data) in &tags {
        if !data.is_empty() {
            last_tag_offset += last_tag_size;
            last_tag_size = data.len();
        }
        out.extend_from_slice(&name.to_be_bytes());
        out.extend_from_slice(&(last_tag_offset as u32).to_be_bytes());
        out.extend_from_slice(&(last_tag_size as u32).to_be_bytes());
    }
    for (_, data) in &tags {
        out.extend_from_slice(data);
    }
    debug_assert_eq!(out.len(), profile_size);
    out
}

/// Port of `SkWriteICCProfile(const SkColorSpace*, const skhdr::Metadata* = nullptr)`
/// (SkICC.cpp#L709-L870) for the sRGB-like transfer functions. Returns `None` for no colour
/// space, and for the HDR transfer functions, which are not ported (see the module comment).
// Port of: src/encode/SkICC.cpp#L709-L870 (chrome/m156)
#[doc(alias = "SkWriteICCProfile")]
#[must_use]
pub fn write_icc_profile(color_space: Option<&ColorSpace>) -> Option<Vec<u8>> {
    let cs = color_space?;
    let fn_ = cs.transfer_fn();
    let to_xyzd50 = cs.to_xyzd50();
    if fn_.is_pqish() || fn_.is_hlgish() || fn_.is_pq() || fn_.is_hlg() {
        return None;
    }
    let desc = get_desc_string(&fn_, &to_xyzd50);
    let trc = if fn_.is_srgbish() { Some(fn_) } else { None };
    Some(write_profile(&Profile { to_xyzd50, trc }, &desc))
}

/// Port of `SkWriteICCProfile(const skcms_TransferFunction&, const skcms_Matrix3x3& toXYZD50)`
/// (SkICC.cpp#L872-L874): the profile of `SkColorSpace::MakeRGB(fn, toXYZD50)`.
#[must_use]
pub fn write_icc_profile_for_rgb(fn_: &TransferFunction, to_xyzd50: &Matrix3x3) -> Option<Vec<u8>> {
    let cs = ColorSpace::new_rgb(fn_, to_xyzd50)?;
    write_icc_profile(Some(&cs))
}

/// Port of `SkWriteICCProfile(const skcms_ICCProfile*, const char* desc)` (SkICC.cpp#L564-L707)
/// for a profile that has a matrix to XYZ D50 and, optionally, one parametric curve shared by the
/// three channels (the profile of an sRGB-like colour space, `SkColorSpace::toProfile`). Returns
/// `None` for a profile with other tags (`CICP`, `HAGC`, `A2B`, `B2A`) or with curves that are
/// tables or differ, which this writer does not write.
// Port of: src/encode/SkICC.cpp#L564-L707 (chrome/m156)
#[doc(alias = "SkWriteICCProfile")]
#[must_use]
pub fn write_icc_profile_from_profile(profile: &IccProfile, desc: &str) -> Option<Vec<u8>> {
    if !profile.has_to_xyzd50
        || profile.has_cicp
        || profile.has_hagc
        || profile.has_a2b
        || profile.has_b2a
    {
        return None;
    }
    let trc = if profile.has_trc {
        let (Curve::Parametric(r), Curve::Parametric(g), Curve::Parametric(b)) =
            (&profile.trc[0], &profile.trc[1], &profile.trc[2])
        else {
            return None;
        };
        if r != g || g != b {
            return None;
        }
        Some(*r)
    } else {
        None
    };
    Some(write_profile(
        &Profile {
            to_xyzd50: profile.to_xyzd50,
            trc,
        },
        desc,
    ))
}

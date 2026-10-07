// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skcms/src/skcms_public.h

//! The data types of skcms's public API: matrices, transfer functions, curves, ICC profiles,
//! pixel and alpha formats.

use std::sync::Arc;

/// A row-major 3x3 matrix (ie `vals[row][col]`).
// Port of: modules/skcms/src/skcms_public.h#L51-L53 (chrome/m156)
#[doc(alias = "skcms_Matrix3x3")]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Matrix3x3 {
    pub vals: [[f32; 3]; 3],
}

/// A row-major 3x4 matrix (ie `vals[row][col]`).
// Port of: modules/skcms/src/skcms_public.h#L60-L62 (chrome/m156)
#[doc(alias = "skcms_Matrix3x4")]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Matrix3x4 {
    pub vals: [[f32; 4]; 3],
}

impl Matrix3x3 {
    /// Bitwise equality of all nine entries (the C++ `memcmp`): `-0.0 != 0.0`, `NaN == NaN` when
    /// the bit patterns match.
    #[must_use]
    pub fn bit_eq(&self, other: &Self) -> bool {
        self.vals
            .iter()
            .flatten()
            .zip(other.vals.iter().flatten())
            .all(|(a, b)| a.to_bits() == b.to_bits())
    }
}

impl Matrix3x4 {
    /// Bitwise equality of all twelve entries (the C++ `memcmp`).
    #[must_use]
    pub fn bit_eq(&self, other: &Self) -> bool {
        self.vals
            .iter()
            .flatten()
            .zip(other.vals.iter().flatten())
            .all(|(a, b)| a.to_bits() == b.to_bits())
    }
}

/// A transfer function mapping encoded values to linear values, represented by this
/// 7-parameter piecewise function:
///
/// ```text
///   linear = sign(encoded) *  (c*|encoded| + f)       , 0 <= |encoded| < d
///          = sign(encoded) * ((a*|encoded| + b)^g + e), d <= |encoded|
/// ```
///
/// (A simple gamma transfer function sets g to gamma and a to 1.)
// Port of: modules/skcms/src/skcms_public.h#L71-L73 (chrome/m156)
#[doc(alias = "skcms_TransferFunction")]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TransferFunction {
    pub g: f32,
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
}

impl TransferFunction {
    /// Creates a transfer function from its seven parameters, in the order `g, a, b, c, d, e, f`.
    #[must_use]
    #[allow(clippy::many_single_char_names)] // mirrors the C++ variable names
    pub const fn new(g: f32, a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) -> Self {
        Self {
            g,
            a,
            b,
            c,
            d,
            e,
            f,
        }
    }

    /// The seven parameters as an array, in memory order `[g, a, b, c, d, e, f]`.
    #[must_use]
    pub const fn to_array(&self) -> [f32; 7] {
        [self.g, self.a, self.b, self.c, self.d, self.e, self.f]
    }

    /// Creates a transfer function from `[g, a, b, c, d, e, f]`.
    #[must_use]
    pub const fn from_array(v: [f32; 7]) -> Self {
        Self::new(v[0], v[1], v[2], v[3], v[4], v[5], v[6])
    }

    /// The raw bytes of the seven parameters (native endian), as `memcpy`/`memcmp` would see them.
    #[must_use]
    pub fn to_ne_bytes(&self) -> [u8; 28] {
        let mut out = [0u8; 28];
        for (i, v) in self.to_array().iter().enumerate() {
            out[4 * i..4 * i + 4].copy_from_slice(&v.to_ne_bytes());
        }
        out
    }

    /// Bitwise equality of all seven parameters (the C++ `memcmp`).
    #[must_use]
    pub fn bit_eq(&self, other: &Self) -> bool {
        self.to_array()
            .iter()
            .zip(other.to_array().iter())
            .all(|(a, b)| a.to_bits() == b.to_bits())
    }
}

/// The kind of function stored in a [`TransferFunction`].
// Port of: modules/skcms/src/skcms_public.h#L79-L87 (chrome/m156)
#[doc(alias = "skcms_TFType")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum TfType {
    #[doc(alias = "skcms_TFType_Invalid")]
    Invalid = 0,
    #[doc(alias = "skcms_TFType_sRGBish")]
    SRGBish = 1,
    #[doc(alias = "skcms_TFType_PQish")]
    PQish = 2,
    #[doc(alias = "skcms_TFType_HLGish")]
    HLGish = 3,
    #[doc(alias = "skcms_TFType_HLGinvish")]
    HLGinvish = 4,
    #[doc(alias = "skcms_TFType_PQ")]
    PQ = 5,
    #[doc(alias = "skcms_TFType_HLG")]
    HLG = 6,
}

/// A shared, immutable byte buffer plus an offset into it: the safe replacement for the
/// `const uint8_t*` pointers that skcms's curves, CLUTs and HAGC data hold into an ICC profile.
#[derive(Clone, Debug)]
pub struct ByteView {
    data: Arc<[u8]>,
    offset: usize,
}

impl ByteView {
    /// A view of `data` starting at `offset`.
    #[must_use]
    pub fn new(data: Arc<[u8]>, offset: usize) -> Self {
        Self { data, offset }
    }

    /// The bytes from the view's start to the end of the underlying buffer.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        self.data.get(self.offset..).unwrap_or(&[])
    }

    /// Byte at `index` (relative to the view's start), or 0 past the end of the buffer.
    #[must_use]
    pub fn byte(&self, index: usize) -> u8 {
        self.as_slice().get(index).copied().unwrap_or(0)
    }

    /// Pointer-style identity: same underlying buffer and same offset.
    #[must_use]
    pub fn same_as(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data) && self.offset == other.offset
    }
}

/// Unified representation of 'curv' or 'para' tag data, or a 1D table from 'mft1' or 'mft2'.
// Port of: modules/skcms/src/skcms_public.h#L160-L173 (chrome/m156)
#[doc(alias = "skcms_Curve")]
#[derive(Clone, Debug)]
pub enum Curve {
    /// A parametric function (`table_entries == 0` in skcms).
    Parametric(TransferFunction),
    /// A table of `entries` 8-bit values (`table_8` in skcms).
    Table8 { entries: u32, table: ByteView },
    /// A table of `entries` big-endian 16-bit values (`table_16` in skcms).
    Table16 { entries: u32, table: ByteView },
}

impl Default for Curve {
    /// An all-zero parametric curve (a zeroed `skcms_Curve`).
    fn default() -> Self {
        Self::Parametric(TransferFunction::default())
    }
}

impl Curve {
    /// `table_entries`: 0 for a parametric curve.
    #[must_use]
    pub fn table_entries(&self) -> u32 {
        match self {
            Self::Parametric(_) => 0,
            Self::Table8 { entries, .. } | Self::Table16 { entries, .. } => *entries,
        }
    }

    /// The parametric function, if this curve is parametric.
    #[must_use]
    pub fn parametric(&self) -> Option<&TransferFunction> {
        match self {
            Self::Parametric(tf) => Some(tf),
            _ => None,
        }
    }

    /// Bitwise (`memcmp`) equality, with table pointers compared by identity.
    #[must_use]
    pub fn bit_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Parametric(a), Self::Parametric(b)) => a.bit_eq(b),
            (
                Self::Table8 {
                    entries: ea,
                    table: ta,
                },
                Self::Table8 {
                    entries: eb,
                    table: tb,
                },
            )
            | (
                Self::Table16 {
                    entries: ea,
                    table: ta,
                },
                Self::Table16 {
                    entries: eb,
                    table: tb,
                },
            ) => ea == eb && ta.same_as(tb),
            _ => false,
        }
    }
}

/// Complex transforms between device space (A) and profile connection space (B):
///
/// ```text
///   A2B:  device -> [ "A" curves -> CLUT ] -> [ "M" curves -> matrix ] -> "B" curves -> PCS
/// ```
// Port of: modules/skcms/src/skcms_public.h#L179-L199 (chrome/m156)
#[doc(alias = "skcms_A2B")]
#[derive(Clone, Debug, Default)]
pub struct A2B {
    /// Optional: N 1D "A" curves, followed by an N-dimensional CLUT.
    /// If `input_channels == 0`, these curves and CLUT are skipped,
    /// otherwise, `input_channels` must be in `[1, 4]`.
    pub input_curves: [Curve; 4],
    pub grid_8: Option<ByteView>,
    pub grid_16: Option<ByteView>,
    pub input_channels: u32,
    pub grid_points: [u8; 4],

    /// Optional: 3 1D "M" curves, followed by a color matrix.
    /// If `matrix_channels == 0`, these curves and matrix are skipped,
    /// otherwise, `matrix_channels` must be 3.
    pub matrix_curves: [Curve; 3],
    pub matrix: Matrix3x4,
    pub matrix_channels: u32,

    /// Required: 3 1D "B" curves. Always present, and `output_channels` must be 3.
    pub output_channels: u32,
    pub output_curves: [Curve; 3],
}

/// The B2A counterpart of [`A2B`]:
///
/// ```text
///   B2A:  device <- [ "A" curves <- CLUT ] <- [ "M" curves <- matrix ] <- "B" curves <- PCS
/// ```
// Port of: modules/skcms/src/skcms_public.h#L201-L221 (chrome/m156)
#[doc(alias = "skcms_B2A")]
#[derive(Clone, Debug, Default)]
pub struct B2A {
    /// Required: 3 1D "B" curves. Always present, and `input_channels` must be 3.
    pub input_curves: [Curve; 3],
    pub input_channels: u32,

    /// Optional: a color matrix, followed by 3 1D "M" curves.
    /// If `matrix_channels == 0`, this matrix and these curves are skipped,
    /// otherwise, `matrix_channels` must be 3.
    pub matrix_channels: u32,
    pub matrix_curves: [Curve; 3],
    pub matrix: Matrix3x4,

    /// Optional: an N-dimensional CLUT, followed by N 1D "A" curves.
    /// If `output_channels == 0`, this CLUT and these curves are skipped,
    /// otherwise, `output_channels` must be in `[1, 4]`.
    pub output_curves: [Curve; 4],
    pub grid_8: Option<ByteView>,
    pub grid_16: Option<ByteView>,
    pub grid_points: [u8; 4],
    pub output_channels: u32,
}

/// Coding-independent code points (CICP) of an ICC profile.
// Port of: modules/skcms/src/skcms_public.h#L223-L228 (chrome/m156)
#[doc(alias = "skcms_CICP")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cicp {
    pub color_primaries: u8,
    pub transfer_characteristics: u8,
    pub matrix_coefficients: u8,
    pub video_full_range_flag: u8,
}

/// The 'hagc' (HDR gain curve) tag data of an ICC profile.
// Port of: modules/skcms/src/skcms_public.h#L230-L233 (chrome/m156)
#[doc(alias = "skcms_HAGC")]
#[derive(Clone, Debug, Default)]
pub struct Hagc {
    pub size: u32,
    pub buffer: Option<ByteView>,
}

/// A parsed ICC profile.
// Port of: modules/skcms/src/skcms_public.h#L235-L277 (chrome/m156)
#[doc(alias = "skcms_ICCProfile")]
#[derive(Clone, Debug, Default)]
#[allow(clippy::struct_excessive_bools)] // mirrors the C++ struct
pub struct IccProfile {
    /// The profile's bytes. [`parse`](crate::parse) copies the input buffer, so the profile
    /// owns it and curves/CLUTs index into it.
    pub buffer: Option<Arc<[u8]>>,

    pub size: u32,
    pub data_color_space: u32,
    pub pcs: u32,
    pub tag_count: u32,

    /// [`parse`](crate::parse) will set commonly-used fields for you when possible:
    ///
    /// If we can parse red, green and blue transfer curves from the profile,
    /// `trc` will be set to those three curves, and `has_trc` will be true.
    pub trc: [Curve; 3],

    /// If this profile's gamut can be represented by a 3x3 transform to XYZD50,
    /// `parse` sets `to_xyzd50` to that transform and `has_to_xyzd50` to true.
    pub to_xyzd50: Matrix3x3,

    /// If the profile has a valid A2B0 or A2B1 tag, `parse` sets `a2b` to that data, and
    /// `has_a2b` to true.  `parse_with_a2b_priority` does the same following any user-provided
    /// prioritization of A2B0, A2B1, or A2B2.
    pub a2b: A2B,

    /// If the profile has a valid B2A0 or B2A1 tag, `parse` sets `b2a` to that data, and
    /// `has_b2a` to true.
    pub b2a: B2A,

    /// If the profile has a valid CICP tag, `parse` sets `cicp` to that data, and `has_cicp`
    /// to true.
    pub cicp: Cicp,

    /// If the profile has a valid HAGC tag, `parse` sets `hagc` to that data, and `has_hagc`
    /// to true.
    pub hagc: Hagc,

    pub has_trc: bool,
    pub has_to_xyzd50: bool,
    pub has_a2b: bool,
    pub has_b2a: bool,
    pub has_cicp: bool,
    pub has_hagc: bool,
}

impl IccProfile {
    /// A profile for programmatic construction: zeroed, with the RGB data color space and XYZ
    /// PCS. See also [`set_transfer_function`](Self::set_transfer_function) and
    /// [`set_xyzd50`](Self::set_xyzd50).
    // Port of: modules/skcms/src/skcms_public.h#L484-L488 (chrome/m156)
    #[doc(alias = "skcms_Init")]
    #[must_use]
    pub fn new() -> Self {
        Self {
            data_color_space: signature::RGB,
            pcs: signature::XYZ,
            ..Self::default()
        }
    }

    /// Sets all three TRC curves to the parametric function `tf`.
    // Port of: modules/skcms/src/skcms_public.h#L490-L497 (chrome/m156)
    #[doc(alias = "skcms_SetTransferFunction")]
    pub fn set_transfer_function(&mut self, tf: &TransferFunction) {
        self.has_trc = true;
        self.trc.fill(Curve::Parametric(*tf));
    }

    /// Sets the gamut matrix.
    // Port of: modules/skcms/src/skcms_public.h#L499-L502 (chrome/m156)
    #[doc(alias = "skcms_SetXYZD50")]
    pub fn set_xyzd50(&mut self, m: &Matrix3x3) {
        self.has_to_xyzd50 = true;
        self.to_xyzd50 = *m;
    }

    /// Bitwise (`memcmp`) equality of every field, with buffer pointers compared by identity.
    #[must_use]
    pub fn bit_eq(&self, other: &Self) -> bool {
        fn opt_arc_same(a: Option<&Arc<[u8]>>, b: Option<&Arc<[u8]>>) -> bool {
            match (a, b) {
                (None, None) => true,
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                _ => false,
            }
        }
        fn opt_view_same(a: Option<&ByteView>, b: Option<&ByteView>) -> bool {
            match (a, b) {
                (None, None) => true,
                (Some(a), Some(b)) => a.same_as(b),
                _ => false,
            }
        }
        fn curves_eq(a: &[Curve], b: &[Curve]) -> bool {
            a.iter().zip(b).all(|(a, b)| a.bit_eq(b))
        }
        opt_arc_same(self.buffer.as_ref(), other.buffer.as_ref())
            && self.size == other.size
            && self.data_color_space == other.data_color_space
            && self.pcs == other.pcs
            && self.tag_count == other.tag_count
            && curves_eq(&self.trc, &other.trc)
            && self.to_xyzd50.bit_eq(&other.to_xyzd50)
            && curves_eq(&self.a2b.input_curves, &other.a2b.input_curves)
            && opt_view_same(self.a2b.grid_8.as_ref(), other.a2b.grid_8.as_ref())
            && opt_view_same(self.a2b.grid_16.as_ref(), other.a2b.grid_16.as_ref())
            && self.a2b.input_channels == other.a2b.input_channels
            && self.a2b.grid_points == other.a2b.grid_points
            && curves_eq(&self.a2b.matrix_curves, &other.a2b.matrix_curves)
            && self.a2b.matrix.bit_eq(&other.a2b.matrix)
            && self.a2b.matrix_channels == other.a2b.matrix_channels
            && self.a2b.output_channels == other.a2b.output_channels
            && curves_eq(&self.a2b.output_curves, &other.a2b.output_curves)
            && curves_eq(&self.b2a.input_curves, &other.b2a.input_curves)
            && self.b2a.input_channels == other.b2a.input_channels
            && self.b2a.matrix_channels == other.b2a.matrix_channels
            && curves_eq(&self.b2a.matrix_curves, &other.b2a.matrix_curves)
            && self.b2a.matrix.bit_eq(&other.b2a.matrix)
            && curves_eq(&self.b2a.output_curves, &other.b2a.output_curves)
            && opt_view_same(self.b2a.grid_8.as_ref(), other.b2a.grid_8.as_ref())
            && opt_view_same(self.b2a.grid_16.as_ref(), other.b2a.grid_16.as_ref())
            && self.b2a.grid_points == other.b2a.grid_points
            && self.b2a.output_channels == other.b2a.output_channels
            && self.cicp == other.cicp
            && self.hagc.size == other.hagc.size
            && opt_view_same(self.hagc.buffer.as_ref(), other.hagc.buffer.as_ref())
            && self.has_trc == other.has_trc
            && self.has_to_xyzd50 == other.has_to_xyzd50
            && self.has_a2b == other.has_a2b
            && self.has_b2a == other.has_b2a
            && self.has_cicp == other.has_cicp
            && self.has_hagc == other.has_hagc
    }
}

/// Common ICC signature values.
// Port of: modules/skcms/src/skcms_public.h#L338-L369 (chrome/m156)
#[doc(alias = "skcms_Signature")]
pub mod signature {
    // common data_color_space values
    pub const CMYK: u32 = 0x434D_594B;
    pub const GRAY: u32 = 0x4752_4159;
    pub const RGB: u32 = 0x5247_4220;

    // pcs (or data_color_space)
    pub const LAB: u32 = 0x4C61_6220;
    pub const XYZ: u32 = 0x5859_5A20;

    // other, less common data_color_space values
    pub const CIELUV: u32 = 0x4C75_7620;
    pub const YCBCR: u32 = 0x5943_6272;
    pub const CIEYXY: u32 = 0x5978_7920;
    pub const HSV: u32 = 0x4853_5620;
    pub const HLS: u32 = 0x484C_5320;
    pub const CMY: u32 = 0x434D_5920;
    pub const CLR2: u32 = 0x3243_4C52;
    pub const CLR3: u32 = 0x3343_4C52;
    pub const CLR4: u32 = 0x3443_4C52;
    pub const CLR5: u32 = 0x3543_4C52;
    pub const CLR6: u32 = 0x3643_4C52;
    pub const CLR7: u32 = 0x3743_4C52;
    pub const CLR8: u32 = 0x3843_4C52;
    pub const CLR9: u32 = 0x3943_4C52;
    pub const CLR10: u32 = 0x4143_4C52;
    pub const CLR11: u32 = 0x4243_4C52;
    pub const CLR12: u32 = 0x4343_4C52;
    pub const CLR13: u32 = 0x4443_4C52;
    pub const CLR14: u32 = 0x4543_4C52;
    pub const CLR15: u32 = 0x4643_4C52;
}

/// Pixel formats understood by [`transform`](crate::transform). The `Swap` / BGR variants are
/// the odd values in skcms's numbering (the `& 1` "swap red and blue" bit).
// Port of: modules/skcms/src/skcms_public.h#L371-L424 (chrome/m156)
#[doc(alias = "skcms_PixelFormat")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PixelFormat {
    #[doc(alias = "skcms_PixelFormat_A_8")]
    A8,
    #[doc(alias = "skcms_PixelFormat_A_8_")]
    A8Swap,
    #[doc(alias = "skcms_PixelFormat_G_8")]
    G8,
    #[doc(alias = "skcms_PixelFormat_G_8_")]
    G8Swap,
    /// Grayscale with alpha.
    #[doc(alias = "skcms_PixelFormat_GA_88")]
    Ga88,
    #[doc(alias = "skcms_PixelFormat_GA_88_")]
    Ga88Swap,

    #[doc(alias = "skcms_PixelFormat_RGB_565")]
    Rgb565,
    #[doc(alias = "skcms_PixelFormat_BGR_565")]
    Bgr565,

    #[doc(alias = "skcms_PixelFormat_ABGR_4444")]
    Abgr4444,
    #[doc(alias = "skcms_PixelFormat_ARGB_4444")]
    Argb4444,

    #[doc(alias = "skcms_PixelFormat_RGB_888")]
    Rgb888,
    #[doc(alias = "skcms_PixelFormat_BGR_888")]
    Bgr888,
    #[doc(alias = "skcms_PixelFormat_RGBA_8888")]
    Rgba8888,
    #[doc(alias = "skcms_PixelFormat_BGRA_8888")]
    Bgra8888,
    /// Automatic sRGB encoding / decoding.
    #[doc(alias = "skcms_PixelFormat_RGBA_8888_sRGB")]
    Rgba8888SRgb,
    /// Automatic sRGB encoding / decoding. (Generally used with linear transfer functions.)
    #[doc(alias = "skcms_PixelFormat_BGRA_8888_sRGB")]
    Bgra8888SRgb,

    #[doc(alias = "skcms_PixelFormat_RGBA_1010102")]
    Rgba1010102,
    #[doc(alias = "skcms_PixelFormat_BGRA_1010102")]
    Bgra1010102,

    /// Little-endian.  Pointers must be 16-bit aligned.
    #[doc(alias = "skcms_PixelFormat_RGB_161616LE")]
    Rgb161616Le,
    #[doc(alias = "skcms_PixelFormat_BGR_161616LE")]
    Bgr161616Le,
    #[doc(alias = "skcms_PixelFormat_RGBA_16161616LE")]
    Rgba16161616Le,
    #[doc(alias = "skcms_PixelFormat_BGRA_16161616LE")]
    Bgra16161616Le,

    /// Big-endian.  Pointers must be 16-bit aligned.
    #[doc(alias = "skcms_PixelFormat_RGB_161616BE")]
    Rgb161616Be,
    #[doc(alias = "skcms_PixelFormat_BGR_161616BE")]
    Bgr161616Be,
    #[doc(alias = "skcms_PixelFormat_RGBA_16161616BE")]
    Rgba16161616Be,
    #[doc(alias = "skcms_PixelFormat_BGRA_16161616BE")]
    Bgra16161616Be,

    /// 1-5-10 half-precision float in [0,1].
    #[doc(alias = "skcms_PixelFormat_RGB_hhh_Norm")]
    RgbHhhNorm,
    #[doc(alias = "skcms_PixelFormat_BGR_hhh_Norm")]
    BgrHhhNorm,
    #[doc(alias = "skcms_PixelFormat_RGBA_hhhh_Norm")]
    RgbaHhhhNorm,
    #[doc(alias = "skcms_PixelFormat_BGRA_hhhh_Norm")]
    BgraHhhhNorm,

    /// 1-5-10 half-precision float.
    #[doc(alias = "skcms_PixelFormat_RGB_hhh")]
    RgbHhh,
    #[doc(alias = "skcms_PixelFormat_BGR_hhh")]
    BgrHhh,
    #[doc(alias = "skcms_PixelFormat_RGBA_hhhh")]
    RgbaHhhh,
    #[doc(alias = "skcms_PixelFormat_BGRA_hhhh")]
    BgraHhhh,

    /// 1-8-23 single-precision float (the normal kind).
    #[doc(alias = "skcms_PixelFormat_RGB_fff")]
    RgbFff,
    #[doc(alias = "skcms_PixelFormat_BGR_fff")]
    BgrFff,
    #[doc(alias = "skcms_PixelFormat_RGBA_ffff")]
    RgbaFfff,
    #[doc(alias = "skcms_PixelFormat_BGRA_ffff")]
    BgraFfff,

    /// Located here to signal no clamping.
    #[doc(alias = "skcms_PixelFormat_RGB_101010x_XR")]
    Rgb101010xXr,
    /// Compatible with `MTLPixelFormatBGR10_XR`.
    #[doc(alias = "skcms_PixelFormat_BGR_101010x_XR")]
    Bgr101010xXr,
    /// Located here to signal no clamping.
    #[doc(alias = "skcms_PixelFormat_RGBA_10101010_XR")]
    Rgba10101010Xr,
    /// Compatible with `MTLPixelFormatBGRA10_XR`.
    #[doc(alias = "skcms_PixelFormat_BGRA_10101010_XR")]
    Bgra10101010Xr,
}

/// How a pixel's alpha channel relates to its color channels.
///
/// We always store any alpha channel linearly. We treat opaque as a strong requirement, not just
/// a performance hint: we will ignore any source alpha and treat it as 1.0, and will make sure
/// that any destination alpha channel is filled with the equivalent of 1.0.
// Port of: modules/skcms/src/skcms_public.h#L436-L443 (chrome/m156)
#[doc(alias = "skcms_AlphaFormat")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlphaFormat {
    /// Alpha is always opaque: `tf-1(r), tf-1(g), tf-1(b), 1.0`.
    #[doc(alias = "skcms_AlphaFormat_Opaque")]
    Opaque,
    /// Alpha and color are unassociated: `tf-1(r), tf-1(g), tf-1(b), a`.
    #[doc(alias = "skcms_AlphaFormat_Unpremul")]
    Unpremul,
    /// Premultiplied while encoded: `tf-1(r)*a, tf-1(g)*a, tf-1(b)*a, a`.
    #[doc(alias = "skcms_AlphaFormat_PremulAsEncoded")]
    PremulAsEncoded,
}

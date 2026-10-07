// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skcms/skcms.cc, modules/skcms/src/skcms_internals.h

//! skcms's portable math helpers, 3x3 matrices and transfer functions.

use crate::public::{Matrix3x3, TfType, TransferFunction};

/// The C++ `(int)x` conversion as x86 performs it (`cvttss2si`): truncates toward zero, and
/// returns `INT_MIN` for NaN and out-of-range values, where C++ has undefined behavior.
#[must_use]
#[allow(clippy::cast_possible_truncation)] // mirrors the C++ casts of the ported arithmetic
pub(crate) fn cvt_i32(x: f32) -> i32 {
    if x.is_nan() || x >= 2_147_483_648.0 || x < -2_147_483_648.0 {
        i32::MIN
    } else {
        // In range after the checks above; `as` truncates toward zero like cvttss2si.
        x as i32
    }
}

// Port of: modules/skcms/src/skcms_internals.h#L153-L156 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // mirrors (float)((int)x)
pub(crate) fn floorf_(x: f32) -> f32 {
    let roundtrip = cvt_i32(x) as f32;
    if roundtrip > x {
        roundtrip - 1.0
    } else {
        roundtrip
    }
}

// Port of: modules/skcms/src/skcms_internals.h#L157 (chrome/m156)
pub(crate) fn fabsf_(x: f32) -> f32 {
    if x < 0.0 { -x } else { x }
}

// Port of: modules/skcms/skcms.cc#L50-L65 (chrome/m156)
#[allow(clippy::cast_precision_loss, clippy::cast_possible_wrap)] // mirrors (float)bits on int32_t
#[allow(clippy::cast_sign_loss)] // mirrors the C++ casts of the ported arithmetic
#[allow(clippy::excessive_precision)] // Skia's float literals kept verbatim
pub(crate) fn log2f_(x: f32) -> f32 {
    // The first approximation of log2(x) is its exponent 'e', minus 127.
    let bits = x.to_bits() as i32;

    let e = bits as f32 * (1.0f32 / (1 << 23) as f32);

    // If we use the mantissa too we can refine the error signficantly.
    let m_bits = (bits & 0x007f_ffff) | 0x3f00_0000;
    let m = f32::from_bits(m_bits as u32);

    e - 124.225_514_990f32 - 1.498_030_302f32 * m - 1.725_879_990f32 / (0.352_088_706_8f32 + m)
}

// Port of: modules/skcms/skcms.cc#L66-L69 (chrome/m156)
#[allow(clippy::approx_constant)] // Skia's float literals kept verbatim
#[allow(clippy::excessive_precision)] // Skia's float literals kept verbatim
pub(crate) fn logf_(x: f32) -> f32 {
    let ln2 = 0.693_147_18f32;
    ln2 * log2f_(x)
}

// Port of: modules/skcms/skcms.cc#L71-L96 (chrome/m156)
#[allow(clippy::cast_precision_loss, clippy::cast_possible_wrap)] // mirrors the C++ casts
#[allow(clippy::cast_sign_loss)] // mirrors the C++ casts of the ported arithmetic
#[allow(clippy::excessive_precision)] // Skia's float literals kept verbatim
pub(crate) fn exp2f_(x: f32) -> f32 {
    if x > 128.0 {
        return f32::INFINITY;
    } else if x < -127.0 {
        return 0.0;
    }
    let fract = x - floorf_(x);

    let fbits = (1.0f32 * (1 << 23) as f32)
        * (x + 121.274_057_500f32 - 1.490_129_070f32 * fract
            + 27.728_023_300f32 / (4.842_525_68f32 - fract));

    // Before we cast fbits to int32_t, check for out of range values to pacify UBSAN.
    // INT_MAX is not exactly representable as a float, so exclude it as effectively infinite.
    // Negative values are effectively underflow - we'll end up returning a (different) negative
    // value, which makes no sense. So clamp to zero.
    if fbits >= i32::MAX as f32 {
        return f32::INFINITY;
    } else if fbits < 0.0 {
        return 0.0;
    }

    let bits = cvt_i32(fbits);
    f32::from_bits(bits as u32)
}

/// Not static in C++, as it's used by some test tools.
// Port of: modules/skcms/skcms.cc#L99-L107 (chrome/m156)
#[must_use]
#[allow(clippy::float_cmp)] // mirrors the C++ exact float comparisons
pub fn powf_(x: f32, y: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    if x == 1.0 {
        return 1.0;
    }
    exp2f_(log2f_(x) * y)
}

// Port of: modules/skcms/skcms.cc#L109-L112 (chrome/m156)
#[allow(clippy::approx_constant)] // Skia's float literals kept verbatim
#[allow(clippy::excessive_precision)] // Skia's float literals kept verbatim
pub(crate) fn expf_(x: f32) -> f32 {
    let log2_e = 1.442_695_040_888_963_4f32;
    exp2f_(log2_e * x)
}

// Port of: modules/skcms/skcms.cc#L114-L115 (chrome/m156)
pub(crate) fn fmaxf_(x: f32, y: f32) -> f32 {
    if x > y { x } else { y }
}
pub(crate) fn fminf_(x: f32, y: f32) -> f32 {
    if x < y { x } else { y }
}

// Port of: modules/skcms/skcms.cc#L117 (chrome/m156)
#[allow(clippy::eq_op, clippy::erasing_op)] // 0 == x*0 is skcms's isfinite
pub(crate) fn isfinitef_(x: f32) -> bool {
    0.0 == x * 0.0
}

// Port of: modules/skcms/skcms.cc#L119-L124 (chrome/m156)
#[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)] // bit-level reinterpretation
pub(crate) fn minus_1_ulp(x: f32) -> f32 {
    let bits = (x.to_bits() as i32).wrapping_sub(1);
    f32::from_bits(bits as u32)
}

pub(crate) fn is_zero_to_one(x: f32) -> bool {
    (0.0..=1.0).contains(&x)
}

// ~~~~ Matrices ~~~~

impl Matrix3x3 {
    /// Inverts the matrix, returning `None` if it is singular or the inverse is not finite.
    /// (Unlike the C++, which says it is _not_ safe to alias in-place, this returns a new matrix.)
    // Port of: modules/skcms/skcms.cc#L1949-L2004 (chrome/m156)
    #[doc(alias = "skcms_Matrix3x3_invert")]
    #[must_use]
    pub fn invert(&self) -> Option<Matrix3x3> {
        let src = self;
        let a00 = f64::from(src.vals[0][0]);
        let a01 = f64::from(src.vals[1][0]);
        let a02 = f64::from(src.vals[2][0]);
        let a10 = f64::from(src.vals[0][1]);
        let a11 = f64::from(src.vals[1][1]);
        let a12 = f64::from(src.vals[2][1]);
        let a20 = f64::from(src.vals[0][2]);
        let a21 = f64::from(src.vals[1][2]);
        let a22 = f64::from(src.vals[2][2]);

        let mut b0 = a00 * a11 - a01 * a10;
        let mut b1 = a00 * a12 - a02 * a10;
        let mut b2 = a01 * a12 - a02 * a11;
        let mut b3 = a20;
        let mut b4 = a21;
        let mut b5 = a22;

        let determinant = b0 * b5 - b1 * b4 + b2 * b3;

        #[allow(clippy::float_cmp)] // mirrors the C++ exact singularity test
        if determinant == 0.0 {
            return None;
        }

        let invdet = 1.0 / determinant;
        #[allow(clippy::cast_possible_truncation)] // mirrors (float)invdet
        if invdet > f64::from(f32::MAX)
            || invdet < -f64::from(f32::MAX)
            || !isfinitef_(invdet as f32)
        {
            return None;
        }

        b0 *= invdet;
        b1 *= invdet;
        b2 *= invdet;
        b3 *= invdet;
        b4 *= invdet;
        b5 *= invdet;

        #[allow(clippy::cast_possible_truncation)] // mirrors the (float) casts
        let dst = Matrix3x3 {
            vals: [
                [
                    (a11 * b5 - a12 * b4) as f32,
                    (a12 * b3 - a10 * b5) as f32,
                    (a10 * b4 - a11 * b3) as f32,
                ],
                [
                    (a02 * b4 - a01 * b5) as f32,
                    (a00 * b5 - a02 * b3) as f32,
                    (a01 * b3 - a00 * b4) as f32,
                ],
                [(b2) as f32, (-b1) as f32, (b0) as f32],
            ],
        };

        for r in 0..3 {
            for c in 0..3 {
                if !isfinitef_(dst.vals[r][c]) {
                    return None;
                }
            }
        }
        Some(dst)
    }

    /// The matrix product `self * other`.
    // Port of: modules/skcms/skcms.cc#L2006-L2015 (chrome/m156)
    #[doc(alias = "skcms_Matrix3x3_concat")]
    #[must_use]
    pub fn concat(&self, other: &Matrix3x3) -> Matrix3x3 {
        let a = self;
        let b = other;
        let mut m = Matrix3x3 {
            vals: [[0.0; 3]; 3],
        };
        for r in 0..3 {
            for c in 0..3 {
                m.vals[r][c] = a.vals[r][0] * b.vals[0][c]
                    + a.vals[r][1] * b.vals[1][c]
                    + a.vals[r][2] * b.vals[2][c];
            }
        }
        m
    }
}

/// Free-function form of [`Matrix3x3::invert`].
#[doc(alias = "skcms_Matrix3x3_invert")]
#[must_use]
pub fn matrix3x3_invert(m: &Matrix3x3) -> Option<Matrix3x3> {
    m.invert()
}

/// Free-function form of [`Matrix3x3::concat`].
#[doc(alias = "skcms_Matrix3x3_concat")]
#[must_use]
pub fn matrix3x3_concat(a: &Matrix3x3, b: &Matrix3x3) -> Matrix3x3 {
    a.concat(b)
}

// Port of: modules/skcms/skcms.cc#L1851 (chrome/m156)
#[derive(Clone, Copy, Debug)]
pub(crate) struct Vector3 {
    pub vals: [f32; 3],
}

// Port of: modules/skcms/skcms.cc#L1853-L1861 (chrome/m156)
pub(crate) fn mv_mul(m: &Matrix3x3, v: &Vector3) -> Vector3 {
    let mut dst = Vector3 { vals: [0.0; 3] };
    for row in 0..3 {
        dst.vals[row] =
            m.vals[row][0] * v.vals[0] + m.vals[row][1] * v.vals[1] + m.vals[row][2] * v.vals[2];
    }
    dst
}

// ~~~~ Transfer functions ~~~~

// Most transfer functions we work with are sRGBish.
// For exotic HDR transfer functions, we encode them using a tf.g that makes no sense,
// and repurpose the other fields to hold the parameters of the HDR functions.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct TfPqish {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
}

// We didn't originally support a scale factor K for HLG, and instead just stored 0 in
// the unused `f` field of skcms_TransferFunction for HLGish and HLGInvish transfer functions.
// By storing f=K-1, those old unusued f=0 values now mean K=1, a noop scale factor.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct TfHlgish {
    pub r: f32,
    pub g: f32,
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub k_minus_1: f32,
}

// Port of: modules/skcms/skcms.cc#L136-L139 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // mirrors -(float)kind
fn tf_kind_marker(kind: TfType) -> f32 {
    // We'd use different NaNs, but those aren't guaranteed to be preserved by WASM.
    -(kind as i32 as f32)
}

// Port of: modules/skcms/skcms.cc#L141-L197 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // mirrors static_cast<int>(tf.g), range-checked first
#[allow(clippy::cast_precision_loss)] // mirrors static_cast<float>(-enum_g)
pub(crate) fn classify(
    tf: &TransferFunction,
    pq: Option<&mut TfPqish>,
    hlg: Option<&mut TfHlgish>,
) -> TfType {
    if tf.g < 0.0 {
        // Negative "g" is mapped to enum values; large negative are for sure invalid.
        if tf.g < -128.0 {
            return TfType::Invalid;
        }
        let enum_g = -(tf.g as i32);
        // Non-whole "g" values are invalid as well.
        #[allow(clippy::float_cmp)] // mirrors the C++ exact test
        if (-enum_g) as f32 != tf.g {
            return TfType::Invalid;
        }
        // TODO: soundness checks for PQ/HLG like we do for sRGBish?
        let fill_hlg = |hlg: Option<&mut TfHlgish>| {
            if let Some(hlg) = hlg {
                *hlg = TfHlgish {
                    r: tf.a,
                    g: tf.b,
                    a: tf.c,
                    b: tf.d,
                    c: tf.e,
                    k_minus_1: tf.f,
                };
            }
        };
        match enum_g {
            x if x == TfType::PQish as i32 => {
                if let Some(pq) = pq {
                    *pq = TfPqish {
                        a: tf.a,
                        b: tf.b,
                        c: tf.c,
                        d: tf.d,
                        e: tf.e,
                        f: tf.f,
                    };
                }
                return TfType::PQish;
            }
            x if x == TfType::HLGish as i32 => {
                fill_hlg(hlg);
                return TfType::HLGish;
            }
            x if x == TfType::HLGinvish as i32 => {
                fill_hlg(hlg);
                return TfType::HLGinvish;
            }
            x if x == TfType::PQ as i32 => {
                #[allow(clippy::float_cmp)] // mirrors the C++ exact tests
                if tf.b != 0.0 || tf.c != 0.0 || tf.d != 0.0 || tf.e != 0.0 || tf.f != 0.0 {
                    return TfType::Invalid;
                }
                return TfType::PQ;
            }
            x if x == TfType::HLG as i32 => {
                #[allow(clippy::float_cmp)] // mirrors the C++ exact tests
                if tf.d != 0.0 || tf.e != 0.0 || tf.f != 0.0 {
                    return TfType::Invalid;
                }
                return TfType::HLG;
            }
            _ => {}
        }
        return TfType::Invalid;
    }

    // Basic soundness checks for sRGBish transfer functions.
    if isfinitef_(tf.a + tf.b + tf.c + tf.d + tf.e + tf.f + tf.g)
        // a,c,d,g should be non-negative to make any sense.
        && tf.a >= 0.0
        && tf.c >= 0.0
        && tf.d >= 0.0
        && tf.g >= 0.0
        // Raising a negative value to a fractional tf->g produces complex numbers.
        && tf.a * tf.d + tf.b >= 0.0
    {
        return TfType::SRGBish;
    }

    TfType::Invalid
}

impl TransferFunction {
    /// Identifies which kind of transfer function is encoded in this [`TransferFunction`].
    // Port of: modules/skcms/skcms.cc#L199-L201 (chrome/m156)
    #[doc(alias = "skcms_TransferFunction_getType")]
    #[must_use]
    pub fn tf_type(&self) -> TfType {
        classify(self, None, None)
    }

    /// Is this an ordinary sRGB-ish transfer function?
    // Port of: modules/skcms/skcms.cc#L202-L204 (chrome/m156)
    #[doc(alias = "skcms_TransferFunction_isSRGBish")]
    #[must_use]
    pub fn is_srgbish(&self) -> bool {
        self.tf_type() == TfType::SRGBish
    }

    /// Is this a PQ-ish transfer function?
    // Port of: modules/skcms/skcms.cc#L205-L207 (chrome/m156)
    #[doc(alias = "skcms_TransferFunction_isPQish")]
    #[must_use]
    pub fn is_pqish(&self) -> bool {
        self.tf_type() == TfType::PQish
    }

    /// Is this an HLG-ish transfer function?
    // Port of: modules/skcms/skcms.cc#L208-L210 (chrome/m156)
    #[doc(alias = "skcms_TransferFunction_isHLGish")]
    #[must_use]
    pub fn is_hlgish(&self) -> bool {
        self.tf_type() == TfType::HLGish
    }

    /// Is this the PQ transfer function?
    // Port of: modules/skcms/skcms.cc#L211-L213 (chrome/m156)
    #[doc(alias = "skcms_TransferFunction_isPQ")]
    #[must_use]
    pub fn is_pq(&self) -> bool {
        self.tf_type() == TfType::PQ
    }

    /// Is this the HLG transfer function?
    // Port of: modules/skcms/skcms.cc#L214-L216 (chrome/m156)
    #[doc(alias = "skcms_TransferFunction_isHLG")]
    #[must_use]
    pub fn is_hlg(&self) -> bool {
        self.tf_type() == TfType::HLG
    }

    /// Makes a `PQish` transfer function:
    ///
    /// ```text
    ///                              max(A + B|encoded|^C, 0)
    ///    linear = sign(encoded) * (------------------------) ^ F
    ///                                  D + E|encoded|^C
    /// ```
    // Port of: modules/skcms/skcms.cc#L218-L224 (chrome/m156)
    #[doc(alias = "skcms_TransferFunction_makePQish")]
    #[must_use]
    #[allow(clippy::many_single_char_names)] // mirrors the C++ variable names
    pub fn make_pqish(a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) -> TransferFunction {
        let tf = TransferFunction::new(tf_kind_marker(TfType::PQish), a, b, c, d, e, f);
        debug_assert!(tf.is_pqish());
        tf
    }

    /// Makes an `HLGish` transfer function with scale factor `k`:
    ///
    /// ```text
    ///            { K * sign(encoded) * ( (R|encoded|)^G )          when 0   <= |encoded| <= 1/R
    ///   linear = { K * sign(encoded) * ( e^(a(|encoded|-c)) + b )  when 1/R <  |encoded|
    /// ```
    // Port of: modules/skcms/skcms.cc#L226-L232 (chrome/m156)
    #[doc(alias = "skcms_TransferFunction_makeScaledHLGish")]
    #[must_use]
    #[allow(clippy::many_single_char_names)] // mirrors the C++ variable names
    pub fn make_scaled_hlgish(k: f32, r: f32, g: f32, a: f32, b: f32, c: f32) -> TransferFunction {
        let tf = TransferFunction::new(tf_kind_marker(TfType::HLGish), r, g, a, b, c, k - 1.0);
        debug_assert!(tf.is_hlgish());
        tf
    }

    /// Compatibility shim with `K=1` for old callers.
    // Port of: modules/skcms/src/skcms_public.h#L110-L115 (chrome/m156)
    #[doc(alias = "skcms_TransferFunction_makeHLGish")]
    #[must_use]
    #[allow(clippy::many_single_char_names)] // mirrors the C++ variable names
    pub fn make_hlgish(r: f32, g: f32, a: f32, b: f32, c: f32) -> TransferFunction {
        Self::make_scaled_hlgish(1.0, r, g, a, b, c)
    }

    /// The PQ transfer function. [`eval`](Self::eval) will always evaluate to the unit PQ EOTF,
    /// which maps [0, 1] to [0, 1], regardless of the other parameters.
    /// This is stored differently from `PQish` transfer functions. In particular:
    ///   - the constant -5 is stored in g
    ///   - the `hdr_reference_white_luminance` parameter is stored in a
    ///   - all other parameters are set to 0
    // Port of: modules/skcms/skcms.cc#L234-L241 (chrome/m156)
    #[doc(alias = "skcms_TransferFunction_makePQ")]
    #[must_use]
    pub fn make_pq(hdr_reference_white_luminance: f32) -> TransferFunction {
        let tf = TransferFunction::new(
            tf_kind_marker(TfType::PQ),
            hdr_reference_white_luminance,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
        );
        debug_assert!(tf.is_pq());
        tf
    }

    /// The HLG transfer function. [`eval`](Self::eval) will always evaluate to the HLG inverse
    /// OETF, which maps [0, 1] to [0, 1], regardless of the other parameters.
    /// This is stored differently from `PQish` transfer functions. In particular:
    ///   - the constant -6 is stored in g
    ///   - the `hdr_reference_white_luminance` parameter is stored in a
    ///   - the `peak_white_luminance` parameter is stored in b
    ///   - the `system_gamma` parameter is stored in c
    ///   - all other parameters are set to 0
    // Port of: modules/skcms/skcms.cc#L243-L254 (chrome/m156)
    #[doc(alias = "skcms_TransferFunction_makeHLG")]
    #[must_use]
    pub fn make_hlg(
        hdr_reference_white_luminance: f32,
        peak_luminance: f32,
        system_gamma: f32,
    ) -> TransferFunction {
        let tf = TransferFunction::new(
            tf_kind_marker(TfType::HLG),
            hdr_reference_white_luminance,
            peak_luminance,
            system_gamma,
            0.0,
            0.0,
            0.0,
        );
        debug_assert!(tf.is_hlg());
        tf
    }

    /// Evaluates the transfer function at `x`.
    // Port of: modules/skcms/skcms.cc#L256-L305 (chrome/m156)
    #[doc(alias = "skcms_TransferFunction_eval")]
    #[must_use]
    #[allow(clippy::excessive_precision)] // Skia's float literals kept verbatim
    pub fn eval(&self, mut x: f32) -> f32 {
        let tf = self;
        let sign = if x < 0.0 { -1.0f32 } else { 1.0f32 };
        x *= sign;

        let mut pq = TfPqish::default();
        let mut hlg = TfHlgish::default();
        match classify(tf, Some(&mut pq), Some(&mut hlg)) {
            TfType::Invalid => {}

            TfType::HLG => {
                let a = 0.178_832_77f32;
                let b = 0.284_668_92f32;
                let c = 0.559_910_73f32;
                return sign
                    * (if x <= 0.5 {
                        x * x / 3.0
                    } else {
                        (expf_((x - c) / a) + b) / 12.0
                    });
            }

            TfType::HLGish => {
                let k = hlg.k_minus_1 + 1.0;
                return k
                    * sign
                    * (if x * hlg.r <= 1.0 {
                        powf_(x * hlg.r, hlg.g)
                    } else {
                        expf_((x - hlg.c) * hlg.a) + hlg.b
                    });
            }

            // TransferFunction::invert() inverts R, G, and a for HLGinvish so this math is fast.
            TfType::HLGinvish => {
                let k = hlg.k_minus_1 + 1.0;
                x /= k;
                return sign
                    * (if x <= 1.0 {
                        hlg.r * powf_(x, hlg.g)
                    } else {
                        hlg.a * logf_(x - hlg.b) + hlg.c
                    });
            }

            TfType::SRGBish => {
                return sign
                    * (if x < tf.d {
                        tf.c * x + tf.f
                    } else {
                        powf_(tf.a * x + tf.b, tf.g) + tf.e
                    });
            }

            TfType::PQ => {
                let c1 = 107.0 / 128.0f32;
                let c2 = 2413.0 / 128.0f32;
                let c3 = 2392.0 / 128.0f32;
                let m1 = 1305.0 / 8192.0f32;
                let m2 = 2523.0 / 32.0f32;
                let p = powf_(x, 1.0f32 / m2);
                return powf_((p - c1) / (c2 - c3 * p), 1.0f32 / m1);
            }

            TfType::PQish => {
                return sign
                    * powf_(
                        (pq.a + pq.b * powf_(x, pq.c)) / (pq.d + pq.e * powf_(x, pq.c)),
                        pq.f,
                    );
            }
        }
        0.0
    }

    /// Inverts the transfer function, returning `None` if it has no (reasonable) inverse.
    // Port of: modules/skcms/skcms.cc#L2020-L2133 (chrome/m156)
    #[doc(alias = "skcms_TransferFunction_invert")]
    #[must_use]
    pub fn invert(&self) -> Option<TransferFunction> {
        let src = self;
        let mut pq = TfPqish::default();
        let mut hlg = TfHlgish::default();
        match classify(src, Some(&mut pq), Some(&mut hlg)) {
            TfType::Invalid | TfType::PQ | TfType::HLG => return None,
            TfType::SRGBish => {} // handled below

            TfType::PQish => {
                return Some(TransferFunction::new(
                    tf_kind_marker(TfType::PQish),
                    -pq.a,
                    pq.d,
                    1.0f32 / pq.f,
                    pq.b,
                    -pq.e,
                    1.0f32 / pq.c,
                ));
            }

            TfType::HLGish => {
                return Some(TransferFunction::new(
                    tf_kind_marker(TfType::HLGinvish),
                    1.0f32 / hlg.r,
                    1.0f32 / hlg.g,
                    1.0f32 / hlg.a,
                    hlg.b,
                    hlg.c,
                    hlg.k_minus_1,
                ));
            }

            TfType::HLGinvish => {
                return Some(TransferFunction::new(
                    tf_kind_marker(TfType::HLGish),
                    1.0f32 / hlg.r,
                    1.0f32 / hlg.g,
                    1.0f32 / hlg.a,
                    hlg.b,
                    hlg.c,
                    hlg.k_minus_1,
                ));
            }
        }

        debug_assert_eq!(classify(src, None, None), TfType::SRGBish);

        // We're inverting this function, solving for x in terms of y.
        //   y = (cx + f)         x < d
        //       (ax + b)^g + e   x ≥ d
        // The inverse of this function can be expressed in the same piecewise form.
        let mut inv = TransferFunction::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);

        // We'll start by finding the new threshold inv.d.
        // In principle we should be able to find that by solving for y at x=d from either side.
        // (If those two d values aren't the same, it's a discontinuous transfer function.)
        let d_l = src.c * src.d + src.f;
        let d_r = powf_(src.a * src.d + src.b, src.g) + src.e;
        if fabsf_(d_l - d_r) > 1.0f32 / 512.0f32 {
            return None;
        }
        inv.d = d_l; // TODO(mtklein): better in practice to choose d_r?

        // When d=0, the linear section collapses to a point.  We leave c,d,f all zero in that case.
        if inv.d > 0.0 {
            // Inverting the linear section is pretty straightfoward:
            //        y       = cx + f
            //        y - f   = cx
            //   (1/c)y - f/c = x
            inv.c = 1.0f32 / src.c;
            inv.f = -src.f / src.c;
        }

        // The interesting part is inverting the nonlinear section:
        //         y                = (ax + b)^g + e.
        //         y - e            = (ax + b)^g
        //        (y - e)^1/g       =  ax + b
        //        (y - e)^1/g - b   =  ax
        //   (1/a)(y - e)^1/g - b/a =   x
        //
        // To make that fit our form, we need to move the (1/a) term inside the exponentiation:
        //   let k = (1/a)^g
        //   (1/a)( y -  e)^1/g - b/a = x
        //        (ky - ke)^1/g - b/a = x

        let k = powf_(src.a, -src.g); // (1/a)^g == a^-g
        inv.g = 1.0f32 / src.g;
        inv.a = k;
        inv.b = -k * src.e;
        inv.e = -src.b / src.a;

        // We need to enforce the same constraints here that we do when fitting a curve,
        // a >= 0 and ad+b >= 0.  These constraints are checked by classify(), so they're true
        // of the source function if we're here.

        // Just like when fitting the curve, there's really no way to rescue a < 0.
        if inv.a < 0.0 {
            return None;
        }
        // On the other hand we can rescue an ad+b that's gone slightly negative here.
        if inv.a * inv.d + inv.b < 0.0 {
            inv.b = -inv.a * inv.d;
        }

        // That should usually make classify(inv) == sRGBish true, but there are a couple situations
        // where we might still fail here, like non-finite parameter values.
        if classify(&inv, None, None) != TfType::SRGBish {
            return None;
        }

        debug_assert!(inv.a >= 0.0);
        debug_assert!(inv.a * inv.d + inv.b >= 0.0);

        // Now in principle we're done.
        // But to preserve the valuable invariant inv(src(1.0f)) == 1.0f, we'll tweak
        // e or f of the inverse, depending on which segment contains src(1.0f).
        let mut s = src.eval(1.0);
        if !isfinitef_(s) {
            return None;
        }

        let sign = if s < 0.0 { -1.0f32 } else { 1.0f32 };
        s *= sign;
        if s < inv.d {
            inv.f = 1.0 - sign * inv.c * s;
        } else {
            inv.e = 1.0 - sign * powf_(inv.a * s + inv.b, inv.g);
        }

        if classify(&inv, None, None) == TfType::SRGBish {
            Some(inv)
        } else {
            None
        }
    }
}

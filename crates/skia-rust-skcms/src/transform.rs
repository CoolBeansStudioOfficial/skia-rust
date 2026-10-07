// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skcms/skcms.cc

//! Building the transform program for a pair of pixel formats and profiles
//! (`skcms_Transform`).

use crate::curve::MAX_TABLE_ENTRIES;
use crate::math::{classify, powf_};
use crate::pipeline::{Arg, Op, Stage, run_program};
use crate::profiles::{
    srgb_inverse_transfer_function, srgb_profile, srgb_transfer_function, xyzd50_profile,
};
use crate::public::{
    AlphaFormat, Curve, IccProfile, Matrix3x3, Matrix3x4, PixelFormat, TfType, TransferFunction,
    signature,
};

// The maximum number of ops in a transform program.
// A complex transform (e.g. A2B to B2A with 4 channels, multi-stage curves, matrices,
// format conversion and colorspace conversion) can require up to ~40 ops.
// 64 gives plenty of padding while keeping stack frames small.
// Port of: modules/skcms/src/skcms_Transform.h#L125 (chrome/m156)
const SKCMS_MAX_PROGRAM_OPS: usize = 64;

// Port of: modules/skcms/skcms.cc#L2521-L2524 (chrome/m156)
#[allow(clippy::float_cmp)] // mirrors the C++ exact comparisons
fn tf_is_gamma(tf: &TransferFunction) -> bool {
    tf.g > 0.0
        && tf.a == 1.0
        && tf.b == 0.0
        && tf.c == 0.0
        && tf.d == 0.0
        && tf.e == 0.0
        && tf.f == 0.0
}

// Port of: modules/skcms/skcms.cc#L2526-L2529 (chrome/m156)
#[derive(Clone, Copy, Debug)]
struct OpAndArg<'a> {
    op: Op,
    arg: Arg<'a>,
}

// Port of: modules/skcms/skcms.cc#L2531-L2569 (chrome/m156)
#[allow(clippy::float_cmp)] // mirrors the C++ exact comparison with 1
#[allow(clippy::if_not_else)] // mirrors the C++ branch order
#[allow(clippy::match_same_arms)] // one arm per op/format, as in the C++ switch
fn select_curve_op(curve: &Curve, channel: usize) -> OpAndArg<'_> {
    struct OpType {
        s_gamma: Op,
        s_rgbish: Op,
        pqish: Op,
        hlgish: Op,
        hlginvish: Op,
        table: Op,
    }
    const K_OPS: [OpType; 4] = [
        OpType {
            s_gamma: Op::GammaR,
            s_rgbish: Op::TfR,
            pqish: Op::PqR,
            hlgish: Op::HlgR,
            hlginvish: Op::HlginvR,
            table: Op::TableR,
        },
        OpType {
            s_gamma: Op::GammaG,
            s_rgbish: Op::TfG,
            pqish: Op::PqG,
            hlgish: Op::HlgG,
            hlginvish: Op::HlginvG,
            table: Op::TableG,
        },
        OpType {
            s_gamma: Op::GammaB,
            s_rgbish: Op::TfB,
            pqish: Op::PqB,
            hlgish: Op::HlgB,
            hlginvish: Op::HlginvB,
            table: Op::TableB,
        },
        OpType {
            s_gamma: Op::GammaA,
            s_rgbish: Op::TfA,
            pqish: Op::PqA,
            hlgish: Op::HlgA,
            hlginvish: Op::HlginvA,
            table: Op::TableA,
        },
    ];
    let op = &K_OPS[channel];

    if let Curve::Parametric(tf) = curve {
        // doesn't matter which op
        let noop = OpAndArg {
            op: Op::LoadA8,
            arg: Arg::None,
        };

        if tf_is_gamma(tf) {
            return if tf.g != 1.0 {
                OpAndArg {
                    op: op.s_gamma,
                    arg: Arg::Tf(tf),
                }
            } else {
                noop
            };
        }

        return match classify(tf, None, None) {
            TfType::Invalid => noop,
            // TODO(https://issues.skia.org/issues/420956739): Consider adding
            // support for PQ and HLG. Generally any code that goes through this
            // path would also want tone mapping too.
            TfType::PQ | TfType::HLG => noop,
            TfType::SRGBish => OpAndArg {
                op: op.s_rgbish,
                arg: Arg::Tf(tf),
            },
            TfType::PQish => OpAndArg {
                op: op.pqish,
                arg: Arg::Tf(tf),
            },
            TfType::HLGish => OpAndArg {
                op: op.hlgish,
                arg: Arg::Tf(tf),
            },
            TfType::HLGinvish => OpAndArg {
                op: op.hlginvish,
                arg: Arg::Tf(tf),
            },
        };
    }
    // The table_* ops make this assumption.
    debug_assert!(curve.table_entries() <= MAX_TABLE_ENTRIES);
    OpAndArg {
        op: op.table,
        arg: Arg::Curve(curve),
    }
}

// Returns negative if any of the curves are malformed.
// Port of: modules/skcms/skcms.cc#L2572-L2617 (chrome/m156)
fn select_curve_ops<'a>(curves: &'a [Curve], num_channels: usize, ops: &mut [OpAndArg<'a>]) -> i32 {
    // We process the channels in reverse order, yielding ops in ABGR order.
    // (Working backwards allows us to fuse trailing B+G+R ops into a single RGB op.)
    let mut cursor = 0usize;
    for index in (0..num_channels).rev() {
        if curves[index].table_entries() > MAX_TABLE_ENTRIES {
            return -1;
        }
        ops[cursor] = select_curve_op(&curves[index], index);
        if !matches!(ops[cursor].arg, Arg::None) {
            cursor += 1;
        }
    }

    // Identify separate B+G+R ops and fuse them into a single RGB op.
    if cursor >= 3 {
        struct FusableOps {
            r: Op,
            g: Op,
            b: Op,
            rgb: Op,
        }
        const K_FUSABLE_OPS: [FusableOps; 5] = [
            FusableOps {
                r: Op::GammaR,
                g: Op::GammaG,
                b: Op::GammaB,
                rgb: Op::GammaRgb,
            },
            FusableOps {
                r: Op::TfR,
                g: Op::TfG,
                b: Op::TfB,
                rgb: Op::TfRgb,
            },
            FusableOps {
                r: Op::PqR,
                g: Op::PqG,
                b: Op::PqB,
                rgb: Op::PqRgb,
            },
            FusableOps {
                r: Op::HlgR,
                g: Op::HlgG,
                b: Op::HlgB,
                rgb: Op::HlgRgb,
            },
            FusableOps {
                r: Op::HlginvR,
                g: Op::HlginvG,
                b: Op::HlginvB,
                rgb: Op::HlginvRgb,
            },
        ];

        let pos_r = cursor - 1;
        let pos_g = cursor - 2;
        let pos_b = cursor - 3;
        for fusable_op in &K_FUSABLE_OPS {
            if ops[pos_r].op == fusable_op.r
                && ops[pos_g].op == fusable_op.g
                && ops[pos_b].op == fusable_op.b
                && tf_args_equal(&ops[pos_r].arg, &ops[pos_g].arg)
                && tf_args_equal(&ops[pos_r].arg, &ops[pos_b].arg)
            {
                // Fuse the three matching ops into one.
                ops[pos_b].op = fusable_op.rgb;
                cursor -= 2;
                break;
            }
        }
    }

    i32::try_from(cursor).unwrap_or(i32::MAX)
}

// memcmp of two transfer function arguments.
fn tf_args_equal(a: &Arg<'_>, b: &Arg<'_>) -> bool {
    match (a, b) {
        (Arg::Tf(a), Arg::Tf(b)) => a.bit_eq(b),
        _ => false,
    }
}

// Port of: modules/skcms/skcms.cc#L2619-L2645 (chrome/m156)
fn bytes_per_pixel(fmt: PixelFormat) -> usize {
    match fmt {
        PixelFormat::A8 | PixelFormat::A8Swap | PixelFormat::G8 | PixelFormat::G8Swap => 1,
        PixelFormat::Ga88
        | PixelFormat::Ga88Swap
        | PixelFormat::Abgr4444
        | PixelFormat::Argb4444
        | PixelFormat::Rgb565
        | PixelFormat::Bgr565 => 2,
        PixelFormat::Rgb888 | PixelFormat::Bgr888 => 3,
        PixelFormat::Rgba8888
        | PixelFormat::Bgra8888
        | PixelFormat::Rgba8888SRgb
        | PixelFormat::Bgra8888SRgb
        | PixelFormat::Rgba1010102
        | PixelFormat::Bgra1010102
        | PixelFormat::Rgb101010xXr
        | PixelFormat::Bgr101010xXr => 4,
        PixelFormat::Rgb161616Le
        | PixelFormat::Bgr161616Le
        | PixelFormat::Rgb161616Be
        | PixelFormat::Bgr161616Be
        | PixelFormat::RgbHhhNorm
        | PixelFormat::BgrHhhNorm
        | PixelFormat::RgbHhh
        | PixelFormat::BgrHhh => 6,
        PixelFormat::Rgba10101010Xr
        | PixelFormat::Bgra10101010Xr
        | PixelFormat::Rgba16161616Le
        | PixelFormat::Bgra16161616Le
        | PixelFormat::Rgba16161616Be
        | PixelFormat::Bgra16161616Be
        | PixelFormat::RgbaHhhhNorm
        | PixelFormat::BgraHhhhNorm
        | PixelFormat::RgbaHhhh
        | PixelFormat::BgraHhhh => 8,
        PixelFormat::RgbFff | PixelFormat::BgrFff => 12,
        PixelFormat::RgbaFfff | PixelFormat::BgraFfff => 16,
    }
}

// See ITU-T H.273 Table 3 for the full list of codes.
// Port of: modules/skcms/skcms.cc#L2647-L2649 (chrome/m156)
const K_TRANSFER_CICP_ID_PQ: u8 = 16;
const K_TRANSFER_CICP_ID_HLG: u8 = 18;

// Port of: modules/skcms/skcms.cc#L2651-L2654 (chrome/m156)
fn has_cicp_pq_trc(profile: &IccProfile) -> bool {
    profile.has_cicp && profile.cicp.transfer_characteristics == K_TRANSFER_CICP_ID_PQ
}

fn has_cicp_hlg_trc(profile: &IccProfile) -> bool {
    profile.has_cicp && profile.cicp.transfer_characteristics == K_TRANSFER_CICP_ID_HLG
}

// Set tf to be the PQ transfer function, scaled such that 1.0 will map to 10,000 / 203.
// Port of: modules/skcms/skcms.cc#L2662-L2674 (chrome/m156)
fn set_reference_pq_ish_trc() -> TransferFunction {
    // Initialize such that 1.0 maps to 1.0.
    let mut tf = TransferFunction::make_pqish(
        -107.0 / 128.0f32,
        1.0,
        32.0 / 2523.0f32,
        2413.0 / 128.0f32,
        -2392.0 / 128.0f32,
        8192.0 / 1305.0f32,
    );

    // Distribute scaling factor W by scaling A and B with X ^ (1/F):
    // ((A + Bx^C) / (D + Ex^C))^F * W = ((A + Bx^C) / (D + Ex^C) * W^(1/F))^F
    // See https://crbug.com/1058580#c32 for discussion.
    let w = 10000.0f32 / 203.0f32;
    let ws = powf_(w, 1.0f32 / tf.f);
    tf.a *= ws;
    tf.b *= ws;
    // The C++ writes `tf->a = ws * tf->a`; multiplication commutes exactly.
    tf
}

// Set tf to be the HLG inverse OETF, scaled such that 1.0 will map to 1.0.
// While this is one version of HLG, there are many others. A better version
// would be to use the 1,000 nit reference version, but that will require
// adding opt-optical transform support.
// Port of: modules/skcms/skcms.cc#L2680-L2684 (chrome/m156)
#[allow(clippy::excessive_precision)] // Skia's float literals kept verbatim
fn set_sdr_hlg_ish_trc() -> TransferFunction {
    let mut tf = TransferFunction::make_hlgish(
        2.0,
        2.0,
        1.0f32 / 0.178_832_77f32,
        0.284_668_92,
        0.559_910_73,
    );
    tf.f = 1.0f32 / 12.0f32 - 1.0f32;
    tf
}

/// What a destination profile needs: the XYZD50-to-profile matrix, the inverse transfer
/// functions, and which path it takes.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DestinationPrep {
    pub from_xyzd50: Matrix3x3,
    pub inv: [TransferFunction; 3],
    pub using_b2a: bool,
    pub using_hlg_ootf: bool,
}

// Port of: modules/skcms/skcms.cc#L2686-L2737 (chrome/m156)
pub(crate) fn prep_for_destination(profile: &IccProfile) -> Option<DestinationPrep> {
    let from = if profile.has_to_xyzd50 {
        profile.to_xyzd50.invert()
    } else {
        None
    };
    let has_xyzd50 = from.is_some();
    let mut prep = DestinationPrep {
        from_xyzd50: from.unwrap_or_default(),
        inv: [TransferFunction::default(); 3],
        using_b2a: false,
        using_hlg_ootf: false,
    };

    // CICP-specified PQ or HLG transfer functions take precedence.
    // TODO: Add the ability to parse CICP primaries to not require
    // the XYZD50 matrix.
    if has_cicp_pq_trc(profile) && has_xyzd50 {
        let trc_pq = set_reference_pq_ish_trc();
        // The C++ ignores the results of these inversions.
        for inv in &mut prep.inv {
            if let Some(v) = trc_pq.invert() {
                *inv = v;
            }
        }
        return Some(prep);
    }
    if has_cicp_hlg_trc(profile) && has_xyzd50 {
        let trc_hlg = set_sdr_hlg_ish_trc();
        for inv in &mut prep.inv {
            if let Some(v) = trc_hlg.invert() {
                *inv = v;
            }
        }
        prep.using_hlg_ootf = true;
        return Some(prep);
    }

    // Then prefer the B2A transformation.
    // skcms_Transform() supports B2A destinations.
    if profile.has_b2a {
        prep.using_b2a = true;
        return Some(prep);
    }

    // Finally use parametric transfer functions.
    // TODO: Reject non sRGB-ish transfer functions here.
    if !(has_xyzd50 && profile.has_trc) {
        return None;
    }
    for i in 0..3 {
        let Curve::Parametric(tf) = &profile.trc[i] else {
            return None;
        };
        prep.inv[i] = tf.invert()?;
    }
    Some(prep)
}

const IDENTITY_3X4: Matrix3x4 = Matrix3x4 {
    vals: [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ],
};

/// Converts `npixels` pixels from the `src` format and color profile to the `dst` format and
/// color profile and returns true, otherwise returns false.
///
/// A `None` profile means sRGB; passing `None` for both is handy when doing format conversion.
/// To transform in place, see [`transform_in_place`].
// Port of: modules/skcms/skcms.cc#L2739-L3134 (chrome/m156)
#[doc(alias = "skcms_Transform")]
#[must_use]
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
pub fn transform(
    src: &[u8],
    src_fmt: PixelFormat,
    src_alpha: AlphaFormat,
    src_profile: Option<&IccProfile>,
    dst: &mut [u8],
    dst_fmt: PixelFormat,
    dst_alpha: AlphaFormat,
    dst_profile: Option<&IccProfile>,
    nz: usize,
) -> bool {
    transform_impl(
        src,
        src_fmt,
        src_alpha,
        src_profile,
        dst,
        dst_fmt,
        dst_alpha,
        dst_profile,
        nz,
    )
}

/// Like [`transform`], but with the source and destination being the same buffer. As in
/// skcms, this is only possible when both pixel formats have the same size.
// Port of: modules/skcms/skcms.cc#L2739-L2762 (chrome/m156)
#[doc(alias = "skcms_Transform")]
#[must_use]
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
pub fn transform_in_place(
    buf: &mut [u8],
    src_fmt: PixelFormat,
    src_alpha: AlphaFormat,
    src_profile: Option<&IccProfile>,
    dst_fmt: PixelFormat,
    dst_alpha: AlphaFormat,
    dst_profile: Option<&IccProfile>,
    nz: usize,
) -> bool {
    // We can't transform in place unless the PixelFormats are the same size.
    if bytes_per_pixel(dst_fmt) != bytes_per_pixel(src_fmt) {
        return false;
    }
    // Every pixel is fully loaded before it is stored, so a copy of the source is equivalent.
    let src = buf.to_vec();
    transform_impl(
        &src,
        src_fmt,
        src_alpha,
        src_profile,
        buf,
        dst_fmt,
        dst_alpha,
        dst_profile,
        nz,
    )
}

#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
#[allow(clippy::too_many_lines)] // one function, as in C++
#[allow(clippy::cast_sign_loss)] // mirrors the C++ casts of the ported arithmetic
#[allow(clippy::match_same_arms)] // one arm per op/format, as in the C++ switch
fn transform_impl(
    src: &[u8],
    src_fmt: PixelFormat,
    mut src_alpha: AlphaFormat,
    src_profile: Option<&IccProfile>,
    dst: &mut [u8],
    dst_fmt: PixelFormat,
    mut dst_alpha: AlphaFormat,
    dst_profile: Option<&IccProfile>,
    nz: usize,
) -> bool {
    let dst_bpp = bytes_per_pixel(dst_fmt);
    let src_bpp = bytes_per_pixel(src_fmt);
    // Let's just refuse if the request is absurdly big.
    let int_max = i32::MAX as usize;
    if nz.saturating_mul(dst_bpp) > int_max || nz.saturating_mul(src_bpp) > int_max {
        return false;
    }
    let n = nz;
    // The C++ trusts the caller; we check the buffers are large enough.
    if src.len() < n * src_bpp || dst.len() < n * dst_bpp {
        return false;
    }

    // Null profiles default to sRGB. Passing null for both is handy when doing format conversion.
    let src_profile = src_profile.unwrap_or_else(|| srgb_profile());
    let mut dst_profile = dst_profile.unwrap_or_else(|| srgb_profile());

    // TODO: more careful alias rejection (like, dst == src + 1)?

    // Locals that program stages borrow, declared before the program.
    // If the source has a TRC that is specified by CICP and not the TRC
    // entries, then store it here for future use.
    let src_cicp_trc;
    // These are always parametric curves of some sort.
    let dst_curves: [Curve; 3];
    // This will store the XYZD50 to destination gamut conversion matrix, if it is needed.
    let dst_from_xyz;
    // This will store the full source to destination gamut conversion matrix, if it is needed.
    let dst_from_src;
    let gray_dst_profile;

    let mut program: Vec<Stage<'_>> = Vec::with_capacity(SKCMS_MAX_PROGRAM_OPS);
    let mut num_ops = 0usize;

    // `add_op` and `add_op_ctx` count every op, but only keep the first SKCMS_MAX_PROGRAM_OPS.
    macro_rules! add_op_ctx {
        ($op:expr, $arg:expr) => {{
            if program.len() < SKCMS_MAX_PROGRAM_OPS {
                program.push(Stage { op: $op, arg: $arg });
            }
            num_ops += 1;
        }};
    }
    macro_rules! add_op {
        ($op:expr) => {
            add_op_ctx!($op, Arg::None)
        };
    }

    macro_rules! add_curve_ops {
        ($curves:expr, $num_channels:expr) => {{
            let mut oa = [OpAndArg {
                op: Op::LoadA8,
                arg: Arg::None,
            }; 4];
            let num_channels: usize = $num_channels;
            debug_assert!(num_channels <= oa.len());

            let num_ops_selected = select_curve_ops($curves, num_channels, &mut oa);
            if num_ops_selected < 0 {
                return false;
            }

            for entry in oa.iter().take(num_ops_selected as usize) {
                add_op_ctx!(entry.op, entry.arg);
            }
        }};
    }

    match src_fmt {
        PixelFormat::A8 | PixelFormat::A8Swap => add_op!(Op::LoadA8),
        PixelFormat::G8 | PixelFormat::G8Swap => add_op!(Op::LoadG8),
        PixelFormat::Ga88 | PixelFormat::Ga88Swap => add_op!(Op::LoadGa88),
        PixelFormat::Abgr4444 | PixelFormat::Argb4444 => add_op!(Op::Load4444),
        PixelFormat::Rgb565 | PixelFormat::Bgr565 => add_op!(Op::Load565),
        PixelFormat::Rgb888 | PixelFormat::Bgr888 => add_op!(Op::Load888),
        PixelFormat::Rgba8888 | PixelFormat::Bgra8888 => add_op!(Op::Load8888),
        PixelFormat::Rgba1010102 | PixelFormat::Bgra1010102 => add_op!(Op::Load1010102),
        PixelFormat::Rgb101010xXr | PixelFormat::Bgr101010xXr => add_op!(Op::Load101010xXr),
        PixelFormat::Rgba10101010Xr | PixelFormat::Bgra10101010Xr => add_op!(Op::Load10101010Xr),
        PixelFormat::Rgb161616Le | PixelFormat::Bgr161616Le => add_op!(Op::Load161616Le),
        PixelFormat::Rgba16161616Le | PixelFormat::Bgra16161616Le => add_op!(Op::Load16161616Le),
        PixelFormat::Rgb161616Be | PixelFormat::Bgr161616Be => add_op!(Op::Load161616Be),
        PixelFormat::Rgba16161616Be | PixelFormat::Bgra16161616Be => add_op!(Op::Load16161616Be),
        PixelFormat::RgbHhhNorm | PixelFormat::BgrHhhNorm => add_op!(Op::LoadHhh),
        PixelFormat::RgbaHhhhNorm | PixelFormat::BgraHhhhNorm => add_op!(Op::LoadHhhh),
        PixelFormat::RgbHhh | PixelFormat::BgrHhh => add_op!(Op::LoadHhh),
        PixelFormat::RgbaHhhh | PixelFormat::BgraHhhh => add_op!(Op::LoadHhhh),
        PixelFormat::RgbFff | PixelFormat::BgrFff => add_op!(Op::LoadFff),
        PixelFormat::RgbaFfff | PixelFormat::BgraFfff => add_op!(Op::LoadFfff),

        PixelFormat::Rgba8888SRgb | PixelFormat::Bgra8888SRgb => {
            add_op!(Op::Load8888);
            add_op_ctx!(Op::TfRgb, Arg::Tf(srgb_transfer_function()));
        }
    }
    if src_fmt == PixelFormat::RgbHhhNorm || src_fmt == PixelFormat::RgbaHhhhNorm {
        add_op!(Op::Clamp);
    }
    if (src_fmt as u32) & 1 != 0 {
        add_op!(Op::SwapRb);
    }
    // skcms quirk, kept for bit-exactness: the C++ `switch (dstFmt >> 1)` has `case
    // skcms_PixelFormat_G_8:` and `case skcms_PixelFormat_GA_88:`, which are the *unshifted*
    // enum values 2 and 4. So the gray destination handling applies to the GA_88 formats
    // (`dstFmt >> 1 == 2`) and, by accident, to the ABGR/ARGB_4444 formats (`dstFmt >> 1 == 4`),
    // but not to G_8.
    match dst_fmt {
        PixelFormat::Ga88
        | PixelFormat::Ga88Swap
        | PixelFormat::Abgr4444
        | PixelFormat::Argb4444 => {
            // When transforming to gray, stop at XYZ (by setting toXYZ to identity), then transform
            // luminance (Y) by the destination transfer function.
            let mut gray = dst_profile.clone();
            gray.set_xyzd50(&xyzd50_profile().to_xyzd50);
            gray_dst_profile = gray;
            dst_profile = &gray_dst_profile;
        }
        _ => {}
    }

    if src_profile.data_color_space == signature::CMYK {
        // Photoshop creates CMYK images as inverse CMYK.
        // These happen to be the only ones we've _ever_ seen.
        add_op!(Op::Invert);
        // With CMYK, ignore the alpha type, to avoid changing K or conflating CMY with K.
        src_alpha = AlphaFormat::Unpremul;
    }

    if src_alpha == AlphaFormat::Opaque {
        add_op!(Op::ForceOpaque);
    } else if src_alpha == AlphaFormat::PremulAsEncoded {
        add_op!(Op::Unpremul);
    }

    if !std::ptr::eq(dst_profile, src_profile) {
        // Track whether or not the A2B or B2A transforms are used. the CICP
        // values take precedence over A2B and B2A.
        let mut src_using_a2b = false;
        let mut src_using_hlg_ootf = false;

        let Some(prep) = prep_for_destination(dst_profile) else {
            return false;
        };
        let dst_using_b2a = prep.using_b2a;
        let dst_using_hlg_ootf = prep.using_hlg_ootf;
        dst_from_xyz = prep.from_xyzd50;
        dst_curves = [
            Curve::Parametric(prep.inv[0]),
            Curve::Parametric(prep.inv[1]),
            Curve::Parametric(prep.inv[2]),
        ];

        if has_cicp_pq_trc(src_profile) && src_profile.has_to_xyzd50 {
            src_cicp_trc = set_reference_pq_ish_trc();
            add_op_ctx!(Op::PqRgb, Arg::Tf(&src_cicp_trc));
        } else if has_cicp_hlg_trc(src_profile) && src_profile.has_to_xyzd50 {
            src_using_hlg_ootf = true;
            src_cicp_trc = set_sdr_hlg_ish_trc();
            add_op_ctx!(Op::HlgRgb, Arg::Tf(&src_cicp_trc));
        } else if src_profile.has_a2b {
            src_using_a2b = true;
            if src_profile.a2b.input_channels != 0 {
                add_curve_ops!(
                    &src_profile.a2b.input_curves,
                    src_profile.a2b.input_channels as usize
                );
                add_op!(Op::Clamp);
                add_op_ctx!(Op::ClutA2B, Arg::A2B(&src_profile.a2b));
            }

            if src_profile.a2b.matrix_channels == 3 {
                add_curve_ops!(&src_profile.a2b.matrix_curves, 3);

                if !IDENTITY_3X4.bit_eq(&src_profile.a2b.matrix) {
                    add_op_ctx!(Op::Matrix3x4, Arg::Matrix3x4(&src_profile.a2b.matrix));
                }
            }

            if src_profile.a2b.output_channels == 3 {
                add_curve_ops!(&src_profile.a2b.output_curves, 3);
            }

            if src_profile.pcs == signature::LAB {
                add_op!(Op::LabToXyz);
            }
        } else if src_profile.has_trc && src_profile.has_to_xyzd50 {
            add_curve_ops!(&src_profile.trc, 3);
        } else {
            return false;
        }

        // A2B sources are in XYZD50 by now, but TRC sources are still in their original gamut.
        debug_assert!(src_profile.has_a2b || src_profile.has_to_xyzd50);

        if dst_using_b2a {
            // B2A needs its input in XYZD50, so transform TRC sources now.
            if !src_using_a2b {
                add_op_ctx!(Op::Matrix3x3, Arg::Matrix3x3(&src_profile.to_xyzd50));
                // Apply the HLG OOTF in XYZD50 space, if needed.
                if src_using_hlg_ootf {
                    add_op!(Op::HlgOotfScale);
                }
            }

            if dst_profile.pcs == signature::LAB {
                add_op!(Op::XyzToLab);
            }

            if dst_profile.b2a.input_channels == 3 {
                add_curve_ops!(&dst_profile.b2a.input_curves, 3);
            }

            if dst_profile.b2a.matrix_channels == 3 {
                if !IDENTITY_3X4.bit_eq(&dst_profile.b2a.matrix) {
                    add_op_ctx!(Op::Matrix3x4, Arg::Matrix3x4(&dst_profile.b2a.matrix));
                }

                add_curve_ops!(&dst_profile.b2a.matrix_curves, 3);
            }

            if dst_profile.b2a.output_channels != 0 {
                add_op!(Op::Clamp);
                add_op_ctx!(Op::ClutB2A, Arg::B2A(&dst_profile.b2a));

                add_curve_ops!(
                    &dst_profile.b2a.output_curves,
                    dst_profile.b2a.output_channels as usize
                );
            }
        } else {
            // This is a TRC destination.

            // Transform to the destination gamut.
            if src_using_hlg_ootf != dst_using_hlg_ootf {
                // If just the src or the dst has an HLG OOTF then we will apply the OOTF in XYZD50
                // space. If both the src and dst has an HLG OOTF then they will cancel.
                if !src_using_a2b {
                    add_op_ctx!(Op::Matrix3x3, Arg::Matrix3x3(&src_profile.to_xyzd50));
                }
                if src_using_hlg_ootf {
                    add_op!(Op::HlgOotfScale);
                }
                if dst_using_hlg_ootf {
                    add_op!(Op::HlginvOotfScale);
                }
                add_op_ctx!(Op::Matrix3x3, Arg::Matrix3x3(&dst_from_xyz));
            } else if src_using_a2b {
                // If the source is A2B then we are already in XYZD50. Just apply the xyz->dst
                // matrix.
                add_op_ctx!(Op::Matrix3x3, Arg::Matrix3x3(&dst_from_xyz));
            } else {
                let to_xyz = &src_profile.to_xyzd50;
                // There's a chance the source and destination gamuts are identical,
                // in which case we can skip the gamut transform.
                if !dst_profile.to_xyzd50.bit_eq(to_xyz) {
                    // Concat the entire gamut transform into dst_from_src.
                    dst_from_src = dst_from_xyz.concat(to_xyz);
                    add_op_ctx!(Op::Matrix3x3, Arg::Matrix3x3(&dst_from_src));
                }
            }

            // Encode back to dst RGB using its parametric transfer functions.
            let mut oa = [OpAndArg {
                op: Op::LoadA8,
                arg: Arg::None,
            }; 3];
            let num_ops_selected = select_curve_ops(&dst_curves, 3, &mut oa);
            // All dst_curves should be parametric, not table-based, so select_curve_ops should
            // not fail.
            debug_assert!(num_ops_selected >= 0);
            for entry in oa.iter().take(num_ops_selected.max(0) as usize) {
                debug_assert!(!matches!(
                    entry.op,
                    Op::TableR | Op::TableG | Op::TableB | Op::TableA
                ));
                add_op_ctx!(entry.op, entry.arg);
            }
        }
    }

    // Clamp here before premul to make sure we're clamping to normalized values _and_ gamut,
    // not just to values that fit in [0,1].
    //
    // E.g. r = 1.1, a = 0.5 would fit fine in fixed point after premul (ra=0.55,a=0.5),
    // but would be carrying r > 1, which is really unexpected for downstream consumers.
    if dst_fmt < PixelFormat::RgbHhh {
        add_op!(Op::Clamp);
    }

    if dst_profile.data_color_space == signature::CMYK {
        // Photoshop creates CMYK images as inverse CMYK.
        // These happen to be the only ones we've _ever_ seen.
        add_op!(Op::Invert);

        // CMYK has no alpha channel, so make sure dstAlpha is a no-op.
        dst_alpha = AlphaFormat::Unpremul;
    }

    if dst_alpha == AlphaFormat::Opaque {
        add_op!(Op::ForceOpaque);
    } else if dst_alpha == AlphaFormat::PremulAsEncoded {
        add_op!(Op::Premul);
    }
    if (dst_fmt as u32) & 1 != 0 {
        add_op!(Op::SwapRb);
    }
    match dst_fmt {
        PixelFormat::A8 | PixelFormat::A8Swap => add_op!(Op::StoreA8),
        PixelFormat::G8 | PixelFormat::G8Swap => add_op!(Op::StoreG8),
        PixelFormat::Ga88 | PixelFormat::Ga88Swap => add_op!(Op::StoreGa88),
        PixelFormat::Abgr4444 | PixelFormat::Argb4444 => add_op!(Op::Store4444),
        PixelFormat::Rgb565 | PixelFormat::Bgr565 => add_op!(Op::Store565),
        PixelFormat::Rgb888 | PixelFormat::Bgr888 => add_op!(Op::Store888),
        PixelFormat::Rgba8888 | PixelFormat::Bgra8888 => add_op!(Op::Store8888),
        PixelFormat::Rgba1010102 | PixelFormat::Bgra1010102 => add_op!(Op::Store1010102),
        PixelFormat::Rgb161616Le | PixelFormat::Bgr161616Le => add_op!(Op::Store161616Le),
        PixelFormat::Rgba16161616Le | PixelFormat::Bgra16161616Le => {
            add_op!(Op::Store16161616Le);
        }
        PixelFormat::Rgb161616Be | PixelFormat::Bgr161616Be => add_op!(Op::Store161616Be),
        PixelFormat::Rgba16161616Be | PixelFormat::Bgra16161616Be => {
            add_op!(Op::Store16161616Be);
        }
        PixelFormat::RgbHhhNorm | PixelFormat::BgrHhhNorm => add_op!(Op::StoreHhh),
        PixelFormat::RgbaHhhhNorm | PixelFormat::BgraHhhhNorm => add_op!(Op::StoreHhhh),
        PixelFormat::Rgb101010xXr | PixelFormat::Bgr101010xXr => add_op!(Op::Store101010xXr),
        PixelFormat::Rgba10101010Xr | PixelFormat::Bgra10101010Xr => {
            add_op!(Op::Store10101010Xr);
        }
        PixelFormat::RgbHhh | PixelFormat::BgrHhh => add_op!(Op::StoreHhh),
        PixelFormat::RgbaHhhh | PixelFormat::BgraHhhh => add_op!(Op::StoreHhhh),
        PixelFormat::RgbFff | PixelFormat::BgrFff => add_op!(Op::StoreFff),
        PixelFormat::RgbaFfff | PixelFormat::BgraFfff => add_op!(Op::StoreFfff),

        PixelFormat::Rgba8888SRgb | PixelFormat::Bgra8888SRgb => {
            add_op_ctx!(Op::TfRgb, Arg::Tf(srgb_inverse_transfer_function()));
            add_op!(Op::Store8888);
        }
    }

    if num_ops > SKCMS_MAX_PROGRAM_OPS {
        return false;
    }

    run_program(&program, src, dst, n);
    true
}

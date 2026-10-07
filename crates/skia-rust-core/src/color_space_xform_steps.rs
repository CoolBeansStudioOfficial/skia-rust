// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkColorSpaceXformSteps.h, src/core/SkColorSpaceXformSteps.cpp

//! [`ColorSpaceXformSteps`]: the minimal set of steps needed to convert color (and alpha type)
//! between two color spaces. See skia.org/docs/user/color.

use skia_rust_skcms::{Matrix3x3, TfType, TransferFunction};

use crate::alpha_type::AlphaType;
use crate::arena_alloc::ArenaAlloc;
use crate::color_space::{ColorSpace, named_gamut, named_transfer_fn};
use crate::color_space_priv::srgb_singleton;
use crate::floating_point::ieee_float_divide;
use crate::raster_pipeline::{RasterPipeline, Stage, transfer_function_ctx};

/// Which steps are needed.
// Port of: src/core/SkColorSpaceXformSteps.h#L22-L38 (chrome/m156)
#[doc(alias = "SkColorSpaceXformSteps::Flags")]
#[allow(clippy::struct_excessive_bools)] // mirrors the C++ struct of independent step flags
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flags {
    pub unpremul: bool,
    pub linearize: bool,
    pub src_ootf: bool,
    pub gamut_transform: bool,
    pub dst_ootf: bool,
    pub encode: bool,
    pub premul: bool,
}

impl Flags {
    /// The flags as a bit mask.
    // Port of: src/core/SkColorSpaceXformSteps.h#L31-L37 (chrome/m156)
    #[must_use]
    pub const fn mask(&self) -> u32 {
        (if self.unpremul { 1 } else { 0 })
            | (if self.linearize { 2 } else { 0 })
            | (if self.src_ootf { 32 } else { 0 })
            | (if self.gamut_transform { 4 } else { 0 })
            | (if self.dst_ootf { 64 } else { 0 })
            | (if self.encode { 8 } else { 0 })
            | (if self.premul { 16 } else { 0 })
    }
}

/// The steps to convert color from a source color space and alpha type to a destination color
/// space and alpha type.
// Port of: src/core/SkColorSpaceXformSteps.h#L20-L63 (chrome/m156)
#[doc(alias = "SkColorSpaceXformSteps")]
#[derive(Clone, Copy, Debug, Default)]
pub struct ColorSpaceXformSteps {
    pub flags: Flags,

    /// Apply for linearize.
    pub src_tf: TransferFunction,
    /// Apply for encode.
    pub dst_tf_inv: TransferFunction,
    /// Apply this 3x3 *column*-major matrix for `gamut_transform`.
    pub src_to_dst_matrix: [f32; 9],
    /// Apply ootf with these r,g,b coefficients and gamma before `gamut_transform`.
    pub src_ootf: [f32; 4],
    /// Apply ootf with these r,g,b coefficients and gamma after `gamut_transform`.
    pub dst_ootf: [f32; 4],
}

// Compute the Y vector for the HLG OOTF in the primaries of a specified color space. The value
// is specified in Rec2020 primaries in ITU-R BT.2100.
// Port of: src/core/SkColorSpaceXformSteps.cpp#L28-L40 (chrome/m156)
#[allow(clippy::items_after_statements)] // mirrors the C++ local declarations
#[allow(clippy::needless_range_loop)] // mirrors the C++ index loops
fn set_ootf_y(cs: &ColorSpace, y: &mut [f32]) {
    let m = cs.gamut_transform_to(
        &ColorSpace::new_rgb(&named_transfer_fn::LINEAR, &named_gamut::REC2020)
            .expect("the linear transfer function is valid"),
    );
    const Y_REC2020: [f32; 3] = [0.262_700, 0.678_000, 0.059_300];
    for i in 0..3 {
        y[i] = 0.0;
        for j in 0..3 {
            y[i] += m.vals[j][i] * Y_REC2020[j];
        }
    }
}

impl ColorSpaceXformSteps {
    /// Computes the steps to convert from `src` (with alpha type `src_at`) to `dst` (with
    /// `dst_at`). A `None` color space is sRGB for the source, and the source's color space for
    /// the destination.
    // Port of: src/core/SkColorSpaceXformSteps.cpp#L42-L212 (chrome/m156)
    #[must_use]
    #[allow(clippy::too_many_lines)] // mirrors the structure of the C++ constructor
    #[allow(clippy::excessive_precision)] // Skia's float literals kept verbatim
    pub fn new(
        src: Option<&ColorSpace>,
        src_at: AlphaType,
        dst: Option<&ColorSpace>,
        mut dst_at: AlphaType,
    ) -> Self {
        let mut steps = Self::default();

        // Opaque outputs are treated as the same alpha type as the source input.
        // TODO: we'd really like to have a good way of explaining why we think this is useful.
        if dst_at == AlphaType::Opaque {
            dst_at = src_at;
        }

        // We have some options about what to do with null src or dst here.
        // This pair seems to be the most consistent with legacy expectations.
        let src = src.unwrap_or_else(|| srgb_singleton());
        let dst = dst.unwrap_or(src);

        if src.hash() == dst.hash() && src_at == dst_at {
            debug_assert!(ColorSpace::equals(Some(src), Some(dst)));
            return steps;
        }

        let src_trfn = src.transfer_fn();
        let dst_trfn = dst.transfer_fn();

        // The scale factor is the amount that values in linear space will be scaled to accommodate
        // peak luminance and HDR reference white luminance.
        let mut scale_factor = 1.0f32;

        // TODO(https://issues.skia.org/issues/420956739): Inline the constants for PQ and HLG transfer
        // functions when the PQish and HLGish transfer functions are no longer in use.
        let k_pqish = TransferFunction::new(
            -2.0,
            -107.0 / 128.0,
            1.0,
            32.0 / 2523.0,
            2413.0 / 128.0,
            -2392.0 / 128.0,
            8192.0 / 1305.0,
        );
        let k_hlgish = TransferFunction::new(
            -3.0,
            2.0,
            2.0,
            1.0 / 0.178_832_77,
            0.284_668_92,
            0.559_910_73,
            0.0,
        );
        let src_tf_type = src_trfn.tf_type();
        match src_tf_type {
            TfType::PQ => {
                // PQ is always scaled by a peak luminance of 10,000 nits, then divided by the HDR
                // reference white luminance (a).
                scale_factor *= 10000.0 / src_trfn.a;
                // Use the default PQish transfer function.
                steps.src_tf = k_pqish;
                steps.flags.linearize = true;
            }
            TfType::HLG => {
                // HLG is scaled by the peak luminance (b), then divided by the HDR reference white
                // luminance (a).
                scale_factor *= src_trfn.b / src_trfn.a;
                steps.flags.linearize = true;
                // Use the HLGish transfer function scaled by 1/12.
                steps.src_tf = k_hlgish;
                steps.src_tf.f = 1.0 / 12.0 - 1.0;
                // If the system gamma is not 1.0, then compute the parameters for the OOTF.
                #[allow(clippy::float_cmp)] // mirrors the C++ exact comparison
                if src_trfn.c != 1.0 {
                    steps.flags.src_ootf = true;
                    steps.src_ootf[3] = src_trfn.c - 1.0;
                    set_ootf_y(src, &mut steps.src_ootf);
                }
            }
            _ => {
                steps.flags.linearize = !src_trfn.bit_eq(&named_transfer_fn::LINEAR);
                if steps.flags.linearize {
                    steps.src_tf = src.transfer_fn();
                }
            }
        }

        let dst_tf_type = dst_trfn.tf_type();
        match dst_tf_type {
            TfType::PQ => {
                // This is the inverse of the treatment of source PQ.
                scale_factor /= 10000.0 / dst_trfn.a;
                steps.flags.encode = true;
                steps.dst_tf_inv = k_pqish;
                if let Some(inv) = steps.dst_tf_inv.invert() {
                    steps.dst_tf_inv = inv;
                }
            }
            TfType::HLG => {
                // This is the inverse of the treatment of source HLG.
                scale_factor /= dst_trfn.b / dst_trfn.a;
                steps.flags.encode = true;
                steps.dst_tf_inv = k_hlgish;
                steps.dst_tf_inv.f = 1.0 / 12.0 - 1.0;
                if let Some(inv) = steps.dst_tf_inv.invert() {
                    steps.dst_tf_inv = inv;
                }
                #[allow(clippy::float_cmp)] // mirrors the C++ exact comparison
                if dst_trfn.c != 1.0 {
                    steps.flags.dst_ootf = true;
                    steps.dst_ootf[3] = 1.0 / dst_trfn.c - 1.0;
                    set_ootf_y(dst, &mut steps.dst_ootf);
                }
            }
            _ => {
                steps.flags.encode = !dst_trfn.bit_eq(&named_transfer_fn::LINEAR);
                if steps.flags.encode {
                    steps.dst_tf_inv = dst.inv_transfer_fn();
                }
            }
        }

        steps.flags.unpremul = src_at == AlphaType::Premul;
        #[allow(clippy::float_cmp)] // mirrors the C++ exact comparison
        {
            steps.flags.gamut_transform =
                src.to_xyzd50_hash() != dst.to_xyzd50_hash() || scale_factor != 1.0;
        }
        steps.flags.premul = src_at != AlphaType::Opaque && dst_at == AlphaType::Premul;

        if steps.flags.gamut_transform {
            // TODO: switch fSrcToDstMatrix to row-major
            let src_to_dst = src.gamut_transform_to(dst);

            steps.src_to_dst_matrix[0] = src_to_dst.vals[0][0] * scale_factor;
            steps.src_to_dst_matrix[1] = src_to_dst.vals[1][0] * scale_factor;
            steps.src_to_dst_matrix[2] = src_to_dst.vals[2][0] * scale_factor;

            steps.src_to_dst_matrix[3] = src_to_dst.vals[0][1] * scale_factor;
            steps.src_to_dst_matrix[4] = src_to_dst.vals[1][1] * scale_factor;
            steps.src_to_dst_matrix[5] = src_to_dst.vals[2][1] * scale_factor;

            steps.src_to_dst_matrix[6] = src_to_dst.vals[0][2] * scale_factor;
            steps.src_to_dst_matrix[7] = src_to_dst.vals[1][2] * scale_factor;
            steps.src_to_dst_matrix[8] = src_to_dst.vals[2][2] * scale_factor;
        } else {
            #[cfg(debug_assertions)]
            {
                let src_m: Matrix3x3 = src.to_xyzd50();
                let dst_m: Matrix3x3 = dst.to_xyzd50();
                debug_assert!(src_m.bit_eq(&dst_m), "Hash collision");
            }
        }

        // If the source and destination OOTFs cancel each other out, skip both.
        if steps.flags.src_ootf && !steps.flags.gamut_transform && steps.flags.dst_ootf {
            // If there is no gamut transform, then the r,g,b coefficients for the
            // OOTFs must be the same.
            debug_assert!(
                steps.src_ootf[..3]
                    .iter()
                    .zip(&steps.dst_ootf[..3])
                    .all(|(a, b)| a.to_bits() == b.to_bits())
            );
            // If the gammas cancel out, then remove the steps.
            #[allow(clippy::float_cmp)] // mirrors the C++ exact comparison
            if (steps.src_ootf[3] + 1.0) * (steps.dst_ootf[3] + 1.0) == 1.0 {
                steps.flags.src_ootf = false;
                steps.flags.dst_ootf = false;
            }
        }

        // If we linearize then immediately reencode with the same transfer function, skip both.
        if steps.flags.linearize
            && !steps.flags.src_ootf
            && !steps.flags.gamut_transform
            && !steps.flags.dst_ootf
            && steps.flags.encode
            && src.transfer_fn_hash() == dst.transfer_fn_hash()
        {
            #[cfg(debug_assertions)]
            {
                // PQ and HLG types use PQish and HLGish for fSrcTF, so this check is not valid for them.
                if src_tf_type != TfType::PQ && src_tf_type != TfType::HLG {
                    let dst_tf = dst.transfer_fn();
                    for (i, (a, b)) in steps
                        .src_tf
                        .to_array()
                        .iter()
                        .zip(dst_tf.to_array().iter())
                        .enumerate()
                    {
                        #[allow(clippy::float_cmp)] // mirrors the C++ exact comparison
                        {
                            debug_assert!(*a == *b, "Hash collision (tf[{i}])");
                        }
                    }
                }
            }
            steps.flags.linearize = false;
            steps.flags.encode = false;
        }

        // Skip unpremul...premul if there are no non-linear operations between.
        if steps.flags.unpremul
            && !steps.flags.linearize
            && !steps.flags.encode
            && steps.flags.premul
        {
            steps.flags.unpremul = false;
            steps.flags.premul = false;
        }

        steps
    }

    /// True if any step is needed (`explicit operator bool`).
    // Port of: src/core/SkColorSpaceXformSteps.h#L53 (chrome/m156)
    #[must_use]
    pub fn is_needed(&self) -> bool {
        self.flags.mask() != 0
    }

    /// Appends the stages of the steps to `p` (`apply(SkRasterPipeline*)`). The contexts
    /// (transfer functions, matrix, OOTF coefficients) are copied into `alloc`.
    // Port of: src/core/SkColorSpaceXformSteps.cpp#L268-L276 (chrome/m156)
    #[doc(alias = "apply")]
    pub fn apply_to_pipeline<'a>(&self, p: &mut RasterPipeline<'a>, alloc: &'a ArenaAlloc) {
        if self.flags.unpremul {
            p.append(Stage::Unpremul);
        }
        if self.flags.linearize {
            p.append_transfer_function(alloc.make(transfer_function_ctx(&self.src_tf)));
        }
        if self.flags.src_ootf {
            p.append(Stage::Ootf(alloc.make(self.src_ootf)));
        }
        if self.flags.gamut_transform {
            p.append(Stage::Matrix3x3(alloc.make(self.src_to_dst_matrix)));
        }
        if self.flags.dst_ootf {
            p.append(Stage::Ootf(alloc.make(self.dst_ootf)));
        }
        if self.flags.encode {
            p.append_transfer_function(alloc.make(transfer_function_ctx(&self.dst_tf_inv)));
        }
        if self.flags.premul {
            p.append(Stage::Premul);
        }
    }

    /// Applies the steps to one color, `rgba`.
    // Port of: src/core/SkColorSpaceXformSteps.cpp#L214-L266 (chrome/m156)
    #[allow(clippy::needless_range_loop)] // mirrors the C++ index loops
    pub fn apply(&self, rgba: &mut [f32; 4]) {
        if self.flags.unpremul {
            // I don't know why isfinite(x) stopped working on the Chromecast bots...
            #[allow(clippy::eq_op, clippy::erasing_op)] // mirrors the C++ x*0 == 0
            let is_finite = |x: f32| x * 0.0 == 0.0;

            let mut inv_a = ieee_float_divide(1.0, rgba[3]);
            inv_a = if is_finite(inv_a) { inv_a } else { 0.0 };
            rgba[0] *= inv_a;
            rgba[1] *= inv_a;
            rgba[2] *= inv_a;
        }
        if self.flags.linearize {
            rgba[0] = self.src_tf.eval(rgba[0]);
            rgba[1] = self.src_tf.eval(rgba[1]);
            rgba[2] = self.src_tf.eval(rgba[2]);
        }
        if self.flags.src_ootf {
            let y = self.src_ootf[0] * rgba[0]
                + self.src_ootf[1] * rgba[1]
                + self.src_ootf[2] * rgba[2];
            // skia-rust: libm (std::pow on floats is powf)
            let y_to_gamma_minus_1 = y.powf(self.src_ootf[3]);
            rgba[0] *= y_to_gamma_minus_1;
            rgba[1] *= y_to_gamma_minus_1;
            rgba[2] *= y_to_gamma_minus_1;
        }
        if self.flags.gamut_transform {
            let temp = [rgba[0], rgba[1], rgba[2]];
            for i in 0..3 {
                rgba[i] = self.src_to_dst_matrix[i] * temp[0]
                    + self.src_to_dst_matrix[3 + i] * temp[1]
                    + self.src_to_dst_matrix[6 + i] * temp[2];
            }
        }
        if self.flags.dst_ootf {
            let y = self.dst_ootf[0] * rgba[0]
                + self.dst_ootf[1] * rgba[1]
                + self.dst_ootf[2] * rgba[2];
            // skia-rust: libm (std::pow on floats is powf)
            let y_to_gamma_minus_1 = y.powf(self.dst_ootf[3]);
            rgba[0] *= y_to_gamma_minus_1;
            rgba[1] *= y_to_gamma_minus_1;
            rgba[2] *= y_to_gamma_minus_1;
        }
        if self.flags.encode {
            rgba[0] = self.dst_tf_inv.eval(rgba[0]);
            rgba[1] = self.dst_tf_inv.eval(rgba[1]);
            rgba[2] = self.dst_tf_inv.eval(rgba[2]);
        }
        if self.flags.premul {
            rgba[0] *= rgba[3];
            rgba[1] *= rgba[3];
            rgba[2] *= rgba[3];
        }
    }
}

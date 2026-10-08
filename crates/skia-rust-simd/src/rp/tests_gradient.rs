// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Tests of the Phase 3 gradient stages (`rp/tiers/{highp,lowp}/sampling.rs`): `xy_to_radius`,
//! `xy_to_unit_angle`, the gradient lookups, `negate_x`, the 2-point conical stages and
//! `apply_vector_mask`.
//!
//! Known answers worked out by hand from the C++ formulas (`SkRasterPipeline_opts.h`), run on
//! every selection this host can run (see `tests_geometry`). There is no oracle run for these
//! stages (the oracle host is gone); the GMs in `tests/gm` check them against Skia's goldens.

#![cfg_attr(miri, allow(dead_code))]
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::excessive_precision,
    clippy::cast_sign_loss,
    clippy::needless_range_loop
)]

use super::Stage;
use super::contexts::{Conical2PtCtx, EvenlySpaced2StopGradientCtx, GradientCtx};
use super::tests_geometry::{
    assert_lanes, cycled, f32_bytes, halves, highp4, lowp2, run, selections,
};
use crate::tier::Selection;

/// Runs `stages` as a lowp program over `n` lanes with `x`, `y` cycled (`gp` stages: the
/// results are the four 16-bit color registers); `None` on Scalar.
fn lowp_colors(
    stages: &[Stage<'_>],
    sel: Selection,
    x: &[f32],
    y: &[f32],
) -> Option<[Vec<u16>; 4]> {
    let n = sel.tier.lowp_stride()?;
    let input = [cycled(x, n), cycled(y, n)].concat();
    let (lowp, out) = run(stages, sel, false, n, &f32_bytes(&input));
    assert!(lowp, "{sel}: expected a lowp program");
    let h = halves(&out[..8 * n]);
    Some([
        h[..n].to_vec(),
        h[n..2 * n].to_vec(),
        h[2 * n..3 * n].to_vec(),
        h[3 * n..].to_vec(),
    ])
}

fn assert_halves(what: &str, sel: Selection, got: &[u16], want: &[u16]) {
    for (i, g) in got.iter().enumerate() {
        assert_eq!(*g, want[i % want.len()], "{what} on {sel}, lane {i}");
    }
}

#[test]
fn xy_to_radius_known_answers() {
    let (xs, ys) = ([3.0, 0.0, -6.0, 1.0], [4.0, 0.0, 8.0, 1.0]);
    let want = [5.0, 0.0, 10.0, 2.0f32.sqrt()];
    for sel in selections() {
        let (r, g, ..) = highp4(&[Stage::XyToRadius], sel, &xs, &ys);
        assert_lanes("highp xy_to_radius r", sel, &r, &want);
        assert_lanes("highp xy_to_radius g", sel, &g, &ys);
        if let Some((x, y)) = lowp2(&[Stage::XyToRadius], sel, &xs, &ys) {
            assert_lanes("lowp xy_to_radius x", sel, &x, &want);
            assert_lanes("lowp xy_to_radius y", sel, &y, &ys);
        }
    }
}

/// Skia's polynomial, in the same operation order.
fn unit_angle(x: f32, y: f32) -> f32 {
    let (xabs, yabs) = (x.abs(), y.abs());
    let slope = xabs.min(yabs) / xabs.max(yabs);
    let s = slope * slope;
    let mut phi = slope
        * (0.159_121_170_639_991_760_253_906_25_f32
            + s * (-5.185_396_969_318_389_892_578_125e-2_f32
                + s * (2.476_101_927_459_239_959_716_796_875e-2_f32
                    + s * (-7.054_738_234_728_574_752_807_617_187_5e-3_f32))));
    if xabs < yabs {
        phi = 1.0 / 4.0 - phi;
    }
    if x < 0.0 {
        phi = 1.0 / 2.0 - phi;
    }
    if y < 0.0 {
        phi = 1.0 - phi;
    }
    if phi.is_nan() {
        phi = 0.0;
    }
    phi
}

#[test]
fn xy_to_unit_angle_known_answers() {
    // The axes are exact; (0, 0) is a NaN slope and becomes 0.
    let (xs, ys) = (
        [1.0, 0.0, -1.0, 0.0, 0.0, 1.0, -2.0, 3.0],
        [0.0, 1.0, 0.0, -1.0, 0.0, 1.0, 0.5, -7.0],
    );
    let want: Vec<f32> = xs
        .iter()
        .zip(&ys)
        .map(|(&x, &y)| unit_angle(x, y))
        .collect();
    assert_eq!(&want[..5], &[0.0, 0.25, 0.5, 0.75, 0.0]);
    for sel in selections() {
        let (r, g, ..) = highp4(&[Stage::XyToUnitAngle], sel, &xs, &ys);
        assert_lanes("highp xy_to_unit_angle r", sel, &r, &want);
        assert_lanes("highp xy_to_unit_angle g", sel, &g, &ys);
        if let Some((x, y)) = lowp2(&[Stage::XyToUnitAngle], sel, &xs, &ys) {
            assert_lanes("lowp xy_to_unit_angle x", sel, &x, &want);
            assert_lanes("lowp xy_to_unit_angle y", sel, &y, &ys);
        }
    }
}

/// A context with `stops` stops (tables padded to eight entries, as `AppendGradientFillStages`
/// does) from `(factor, bias)` rows (rgba) and `ts`.
fn gradient_ctx(rows: &[([f32; 4], [f32; 4])], ts: &[f32]) -> GradientCtx {
    let len = (rows.len() + 1).max(8);
    let mut ctx = GradientCtx {
        stop_count: rows.len(),
        ts: ts.to_vec(),
        ..GradientCtx::default()
    };
    ctx.ts.resize(len, 0.0);
    for c in 0..4 {
        ctx.factors[c] = rows.iter().map(|r| r.0[c]).collect();
        ctx.biases[c] = rows.iter().map(|r| r.1[c]).collect();
        ctx.factors[c].resize(len, 0.0);
        ctx.biases[c].resize(len, 0.0);
    }
    ctx
}

#[test]
fn evenly_spaced_2_stop_gradient_known_answers() {
    // c_l = (1, 0, 0, 1), c_r = (0, 0.5, 1, 1): factor = c_r - c_l, bias = c_l.
    let ctx = EvenlySpaced2StopGradientCtx {
        factor: [-1.0, 0.5, 1.0, 0.0],
        bias: [1.0, 0.0, 0.0, 1.0],
    };
    let ts = [0.0, 0.25, 0.5, 1.0];
    let want = [
        [1.0, 0.75, 0.5, 0.0],
        [0.0, 0.125, 0.25, 0.5],
        [0.0, 0.25, 0.5, 1.0],
        [1.0, 1.0, 1.0, 1.0],
    ];
    // lowp: trunc(clamp(v) * 255 + 0.5)
    let want8: [[u16; 4]; 4] = [
        [255, 191, 128, 0],
        [0, 32, 64, 128],
        [0, 64, 128, 255],
        [255, 255, 255, 255],
    ];
    for sel in selections() {
        let (r, g, b, a) = highp4(&[Stage::EvenlySpaced2StopGradient(&ctx)], sel, &ts, &ts);
        for (what, got, want) in [
            ("r", r, want[0]),
            ("g", g, want[1]),
            ("b", b, want[2]),
            ("a", a, want[3]),
        ] {
            assert_lanes(&format!("highp 2 stop {what}"), sel, &got, &want);
        }
        if let Some(c) = lowp_colors(&[Stage::EvenlySpaced2StopGradient(&ctx)], sel, &ts, &ts) {
            for k in 0..4 {
                assert_halves(&format!("lowp 2 stop {k}"), sel, &c[k], &want8[k]);
            }
        }
    }
}

/// Red, green, blue at 0, 0.5, 1: segment factors and biases, then the last color constant.
fn evenly_spaced_ctx() -> GradientCtx {
    gradient_ctx(
        &[
            ([-2.0, 2.0, 0.0, 0.0], [1.0, 0.0, 0.0, 1.0]),
            ([0.0, -2.0, 2.0, 0.0], [0.0, 2.0, -1.0, 1.0]),
            ([0.0, 0.0, 0.0, 0.0], [0.0, 0.0, 1.0, 1.0]),
        ],
        &[],
    )
}

#[test]
fn evenly_spaced_gradient_known_answers() {
    let ctx = evenly_spaced_ctx();
    let ts = [0.0, 0.25, 0.5, 0.75, 1.0, 0.125, 0.875, 0.5];
    let want = [
        [1.0, 0.5, 0.0, 0.0, 0.0, 0.75, 0.0, 0.0],
        [0.0, 0.5, 1.0, 0.5, 0.0, 0.25, 0.25, 1.0],
        [0.0, 0.0, 0.0, 0.5, 1.0, 0.0, 0.75, 0.0],
        [1.0; 8],
    ];
    for sel in selections() {
        let (r, g, b, a) = highp4(&[Stage::EvenlySpacedGradient(&ctx)], sel, &ts, &ts);
        for (what, got, want) in [
            ("r", r, want[0]),
            ("g", g, want[1]),
            ("b", b, want[2]),
            ("a", a, want[3]),
        ] {
            assert_lanes(&format!("highp evenly_spaced {what}"), sel, &got, &want);
        }
        if let Some(c) = lowp_colors(&[Stage::EvenlySpacedGradient(&ctx)], sel, &ts, &ts) {
            for k in 0..4 {
                let w8: Vec<u16> = want[k]
                    .iter()
                    .map(|v| (v * 255.0 + 0.5f32).clamp(0.0, 255.5) as u16)
                    .collect();
                assert_halves(&format!("lowp evenly_spaced {k}"), sel, &c[k], &w8);
            }
        }
    }
}

#[test]
fn gradient_known_answers() {
    // Black before 0.25, black -> white up to 0.75, white after.
    let ctx = gradient_ctx(
        &[
            ([0.0; 4], [0.0, 0.0, 0.0, 1.0]),
            ([2.0, 2.0, 2.0, 0.0], [-0.5, -0.5, -0.5, 1.0]),
            ([0.0; 4], [1.0; 4]),
        ],
        &[0.0, 0.25, 0.75],
    );
    let ts = [-1.0, 0.1, 0.25, 0.5, 0.625, 0.75, 1.0, 5.0];
    let rgb = [0.0, 0.0, 0.0, 0.5, 0.75, 1.0, 1.0, 1.0];
    for sel in selections() {
        let (r, g, b, a) = highp4(&[Stage::Gradient(&ctx)], sel, &ts, &ts);
        assert_lanes("highp gradient r", sel, &r, &rgb);
        assert_lanes("highp gradient g", sel, &g, &rgb);
        assert_lanes("highp gradient b", sel, &b, &rgb);
        assert_lanes("highp gradient a", sel, &a, &[1.0]);
        if let Some(c) = lowp_colors(&[Stage::Gradient(&ctx)], sel, &ts, &ts) {
            let w8: Vec<u16> = rgb.iter().map(|v| (v * 255.0 + 0.5f32) as u16).collect();
            for k in 0..3 {
                assert_halves(&format!("lowp gradient {k}"), sel, &c[k], &w8);
            }
            assert_halves("lowp gradient a", sel, &c[3], &[255]);
        }
    }
}

#[test]
fn negate_x_and_unswap_known_answers() {
    let (xs, ys) = ([3.0, -0.5, 0.0, 1.5], [1.0, 2.0, 3.0, 4.0]);
    for sel in selections() {
        let (r, g, ..) = highp4(&[Stage::NegateX], sel, &xs, &ys);
        assert_lanes("negate_x r", sel, &r, &[-3.0, 0.5, -0.0, -1.5]);
        assert_lanes("negate_x g", sel, &g, &ys);
        let (r, ..) = highp4(&[Stage::Alter2ptConicalUnswap], sel, &xs, &ys);
        assert_lanes("unswap r", sel, &r, &[-2.0, 1.5, 1.0, -0.5]);
    }
}

#[test]
fn conical_stages_known_answers() {
    let ctx = Conical2PtCtx {
        p0: 4.0,
        p1: 0.5,
        ..Conical2PtCtx::default()
    };
    // strip: t = x + sqrt(p0 - y*y)
    let (xs, ys) = ([1.0, 2.0, -1.0, 0.0], [0.0, 1.0, 2.0, 3.0]);
    let strip = [3.0, 2.0 + 3.0f32.sqrt(), -1.0, f32::NAN];
    // well behaved (p0 = 4): sqrt(x*x + y*y) - x*p0
    let wb = [1.0 - 4.0, 5.0f32.sqrt() - 8.0, 5.0f32.sqrt() + 4.0, 3.0];
    // greater / smaller: +-sqrt(x*x - y*y) - x*p0
    let gt = [1.0 - 4.0, 3.0f32.sqrt() - 8.0, f32::NAN, f32::NAN];
    let sm = [-1.0 - 4.0, -(3.0f32.sqrt()) - 8.0, f32::NAN, f32::NAN];
    // focal on circle: x + y*y/x
    let foc = [1.0, 2.5, -5.0, f32::INFINITY];
    for sel in selections() {
        for (what, stage, want) in [
            ("strip", Stage::XyTo2ptConicalStrip(&ctx), &strip),
            ("well_behaved", Stage::XyTo2ptConicalWellBehaved(&ctx), &wb),
            ("greater", Stage::XyTo2ptConicalGreater(&ctx), &gt),
            ("smaller", Stage::XyTo2ptConicalSmaller(&ctx), &sm),
            ("focal_on_circle", Stage::XyTo2ptConicalFocalOnCircle, &foc),
        ] {
            let (r, g, ..) = highp4(&[stage], sel, &xs, &ys);
            for (i, (got, want)) in r.iter().zip(want.iter().cycle()).enumerate() {
                let same = (got.is_nan() && want.is_nan()) || got.to_bits() == want.to_bits();
                assert!(same, "{what} on {sel}, lane {i}: got {got}, want {want}");
            }
            assert_lanes(&format!("{what} g"), sel, &g, &ys);
        }
        let (r, ..) = highp4(
            &[Stage::Alter2ptConicalCompensateFocal(&ctx)],
            sel,
            &xs,
            &ys,
        );
        assert_lanes("compensate_focal", sel, &r, &[1.5, 2.5, -0.5, 0.5]);
    }
}

#[test]
fn conical_masks_known_answers() {
    // t = r: NaN (and, for degenerates, t <= 0) lanes are zeroed and masked off.
    let ts = [1.0, f32::NAN, 0.0, -2.0, 0.5, f32::INFINITY];
    let keep_nan = [true, false, true, true, true, true];
    let keep_deg = [true, false, false, false, true, true];
    for sel in selections() {
        for (what, nan_only) in [("nan", true), ("degenerates", false)] {
            let ctx = Conical2PtCtx::default();
            let mask_stage = if nan_only {
                Stage::Mask2ptConicalNan(&ctx)
            } else {
                Stage::Mask2ptConicalDegenerates(&ctx)
            };
            let keep = if nan_only { keep_nan } else { keep_deg };
            // r = t (zeroed where degenerate), then the vector mask is applied to a = 0.75.
            let (r, _, _, a) = highp4(
                &[mask_stage, Stage::ApplyVectorMask(&ctx.mask)],
                sel,
                &ts,
                &ts,
            );
            for (i, (rv, av)) in r.iter().zip(&a).enumerate() {
                let t = ts[i % ts.len()];
                let k = keep[i % keep.len()];
                let want_r = if k { t } else { 0.0 };
                assert_eq!(
                    rv.to_bits(),
                    want_r.to_bits(),
                    "{what} r on {sel}, lane {i}"
                );
                assert_eq!(
                    *av,
                    if k { 0.75 } else { 0.0 },
                    "{what} a on {sel}, lane {i}"
                );
            }
        }
    }
}

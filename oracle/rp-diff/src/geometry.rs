// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! B5 cases: the geometry and tiling stages (`matrix_translate`, `matrix_scale_translate`,
//! `matrix_2x3`, `matrix_perspective`, `repeat_x/y`, `mirror_x/y`, `{clamp,repeat,mirror}_x_1`,
//! `clamp_x_and_y`, `decal_x/y/x_and_y` with `check_decal_mask`).
//!
//! The stages transform `r, g` (highp) or the joined `x = r++g`, `y = b++a` (lowp "GG" stages,
//! which reinterpret the 16-bit registers as `f32` lanes: word `j` of the `load_src` buffer is
//! lane `j` of `x` and word `N + j` lane `j` of `y`, the same words as highp's `r` and `g`), so
//! the register cases of [`crate::cases::Cases::registers`] apply to both; this file adds the
//! coordinate-shaped inputs ([`coord_input`]: multiples of half the tile size and their ulp
//! neighbours) that tiling and decals are sensitive to.

use skia_rust_simd::rp::Op;

use crate::case::{Buffer, Case, Ctx, Rect, StageSpec};
use crate::cases::{Cases, Inputs, Precision, REG_BYTES, Rng, output, register_input};

/// Widths of register cases: a single (tail) pixel and a width with a tail on every stride.
const WIDTHS: &[usize] = &[1, 19];

/// The float after `x` toward +inf (`x` finite).
fn next_up(x: f32) -> f32 {
    let b = x.to_bits();
    if x == 0.0 {
        f32::from_bits(1)
    } else if b >> 31 == 0 {
        f32::from_bits(b + 1)
    } else {
        f32::from_bits(b - 1)
    }
}

/// The float before `x` toward -inf (`x` finite).
fn next_down(x: f32) -> f32 {
    -next_up(-x)
}

/// `REG_BYTES` of coordinates around the multiples of `scale / 2`: each multiple, its ulp
/// neighbours, and a few far, tiny and signed-zero values; `seed` rotates the sequence.
///
/// # Panics
/// Never (the counts are small constants).
#[must_use]
pub fn coord_input(scale: f32, seed: u32) -> Vec<u8> {
    let mut values: Vec<f32> = Vec::new();
    for k in -6i16..=12 {
        let x = f32::from(k) * scale * 0.5;
        values.extend([x, next_up(x), next_down(x)]);
    }
    values.extend([
        -0.0,
        scale * 0.999_999,
        scale * 1.000_001,
        scale * 1000.0,
        -scale * 1000.0,
        scale * 1.0e7,
        f32::from_bits(1),
        -f32::from_bits(1),
    ]);
    let mut rng = Rng::new(seed.wrapping_mul(0x9e37_79b1) | 1);
    let n = u32::try_from(values.len()).expect("few values");
    let start = rng.below(n);
    let step = [1, 5, 7, 11][rng.below(4) as usize];
    (0..REG_BYTES / 4)
        .map(|j| u32::try_from(j).expect("small"))
        .flat_map(|j| {
            values[((start + j * step) % n) as usize]
                .to_bits()
                .to_le_bytes()
        })
        .collect()
}

/// The pipeline `load_src(0) load_dst(1) <stages> store_src(2) store_dst(3)` over `w` pixels.
fn register_case(
    c: &mut Cases,
    name: String,
    highp: bool,
    stages: &[StageSpec],
    src: Vec<u8>,
    dst: Vec<u8>,
    w: usize,
) {
    let mut all = vec![
        StageSpec::with(Op::LoadSrc, Ctx::Ptr { slot: 0, offset: 0 }),
        StageSpec::with(Op::LoadDst, Ctx::Ptr { slot: 1, offset: 0 }),
    ];
    all.extend_from_slice(stages);
    all.push(StageSpec::with(
        Op::StoreSrc,
        Ctx::Ptr { slot: 2, offset: 0 },
    ));
    all.push(StageSpec::with(
        Op::StoreDst,
        Ctx::Ptr { slot: 3, offset: 0 },
    ));
    c.push(Case {
        name,
        force_highp: highp,
        compiled: false,
        buffers: vec![
            Buffer::new(src),
            Buffer::new(dst),
            output(REG_BYTES),
            output(REG_BYTES),
        ],
        stages: all,
        runs: vec![Rect::new(0, 0, w, 1)],
    });
}

/// Register cases of a geometry stage sequence: for each precision, the inputs that make sense
/// for coordinates (`Special`, `NanSrc`, `Unit`, `Bits`, lowp also `Bytes`) and, if `coords` is
/// set, the [`coord_input`]s around multiples of those scales, at [`WIDTHS`].
fn geometry_registers(
    c: &mut Cases,
    group: &str,
    stages: &[StageSpec],
    precisions: &[Precision],
    coords: &[f32],
) {
    geometry_registers_with(c, group, stages, precisions, coords, false);
}

/// The special values of [`float_specials`](crate::cases::float_specials) below `1e30` in
/// magnitude (no infinities), rotated like [`Inputs::Special`].
fn finite_special_input(seed: u32) -> Vec<u8> {
    let s: Vec<u32> = crate::cases::float_specials()
        .into_iter()
        .filter(|&b| f32::from_bits(b).abs() < 1.0e30)
        .collect();
    let mut rng = Rng::new(seed.wrapping_mul(0x85eb_ca6b) | 1);
    let n = u32::try_from(s.len()).expect("few specials");
    let start = rng.below(n);
    let step = [1, 5, 7, 9][rng.below(4) as usize];
    (0..REG_BYTES / 4)
        .map(|j| u32::try_from(j).expect("small"))
        .flat_map(|j| s[((start + j * step) % n) as usize].to_le_bytes())
        .collect()
}

/// [`geometry_registers`]; with `finite`, the `Special` inputs have no infinities. For
/// `matrix_perspective`, whose `x * rcp_precise(z)` lets two different NaNs meet in a `mulps`
/// whenever an infinity meets a zero matrix entry (which NaN wins is the compiler's choice, see
/// [`Inputs`]).
fn geometry_registers_with(
    c: &mut Cases,
    group: &str,
    stages: &[StageSpec],
    precisions: &[Precision],
    coords: &[f32],
    finite: bool,
) {
    for &p in precisions {
        let kinds: &[Inputs] = match p {
            Precision::Highp => &[Inputs::Special, Inputs::NanSrc, Inputs::Unit, Inputs::Bits],
            Precision::Auto => &[
                Inputs::Special,
                Inputs::NanSrc,
                Inputs::Unit,
                Inputs::Bits,
                Inputs::Bytes,
            ],
        };
        for &w in WIDTHS {
            // Registers only: the width changes which chunk `store_src` sees (tail or not), not
            // the arithmetic, so the second width needs only one kind of input.
            let kinds: &[Inputs] = if w == WIDTHS[0] {
                kinds
            } else {
                &[Inputs::Unit]
            };
            for &kind in kinds {
                let (s1, s2) = (c.next_seed(), c.next_seed());
                let (src, dst) = if finite && kind == Inputs::Special {
                    (finite_special_input(s1), finite_special_input(s2))
                } else {
                    kind.pair(s1, s2)
                };
                register_case(
                    c,
                    format!("{group}/{}/{}/w{w}", p.name(), kind.name()),
                    p == Precision::Highp,
                    stages,
                    src,
                    dst,
                    w,
                );
            }
            for (i, &scale) in coords.iter().enumerate().filter(|_| w == WIDTHS[0]) {
                let (s1, s2) = (c.next_seed(), c.next_seed());
                register_case(
                    c,
                    format!("{group}/{}/coords{i}/w{w}", p.name()),
                    p == Precision::Highp,
                    stages,
                    coord_input(scale, s1),
                    register_input(Inputs::Bits, s2),
                    w,
                );
            }
        }
    }
}

fn f32s(v: &[f32]) -> Ctx {
    Ctx::F32(v.to_vec())
}

/// All B5 cases.
pub fn b5_geometry(c: &mut Cases) {
    matrices(c);
    tiles(c);
    clamps(c);
    decals(c);
}

/// `matrix_translate`, `matrix_scale_translate`, `matrix_2x3`, `matrix_perspective`.
fn matrices(c: &mut Cases) {
    let big = 3.0e38_f32;
    let tiny = f32::from_bits(0x0000_0400); // denormal
    let translate: &[&[f32]] = &[
        &[10.5, -3.25],
        &[0.0, 0.0],
        &[-0.0, 0.5],
        &[1.0e30, -1.0e30],
        &[tiny, -tiny],
        &[0.5, 255.5],
    ];
    for (i, m) in translate.iter().enumerate() {
        geometry_registers(
            c,
            &format!("matrix_translate/m{i}"),
            &[StageSpec::with(Op::MatrixTranslate, f32s(m))],
            Precision::BOTH,
            &[],
        );
    }
    let scale_translate: &[&[f32]] = &[
        &[2.0, 3.0, 0.5, -1.0],
        &[1.0, 1.0, 0.0, 0.0],
        &[0.5, -0.5, 100.5, -100.5],
        &[1.0e20, 1.0e-20, 1.0e10, 1.0e-10],
        &[big, -big, big, -big],
        &[-0.0, 0.0, tiny, -tiny],
        &[1.0 / 3.0, 7.0, 0.1, 0.7],
    ];
    for (i, m) in scale_translate.iter().enumerate() {
        geometry_registers(
            c,
            &format!("matrix_scale_translate/m{i}"),
            &[StageSpec::with(Op::MatrixScaleTranslate, f32s(m))],
            Precision::BOTH,
            &[],
        );
    }
    let m2x3: &[&[f32]] = &[
        &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        &[0.8, -0.6, 3.5, 0.6, 0.8, -2.0],
        &[1.0, 0.25, 0.0, 0.0, 1.0, 0.0],
        &[1.0e20, 1.0e20, 1.0e-20, -1.0e20, 1.0e20, 5.0],
        &[big, big, big, big, big, big],
        &[0.1, 0.2, 0.3, 0.4, 0.5, 0.6],
        &[-0.0, tiny, 0.0, -tiny, 0.0, -0.0],
    ];
    for (i, m) in m2x3.iter().enumerate() {
        geometry_registers(
            c,
            &format!("matrix_2x3/m{i}"),
            &[StageSpec::with(Op::Matrix2x3, f32s(m))],
            Precision::BOTH,
            &[],
        );
    }
    // Row-major; the last row is the perspective divisor, whose reciprocal is `rcp_precise`:
    // the estimate plus Newton-Raphson steps (tier-dependent estimates).
    let persp: &[&[f32]] = &[
        &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.001, 0.002, 1.0],
        &[0.5, 0.25, 3.0, -0.75, 1.5, 2.0, 0.01, -0.02, 0.9],
        // The divisor crosses zero.
        &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.5, -0.5, 0.0],
        &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0],
        &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 0.0, 0.0, tiny],
        &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 0.0, 0.0, big],
        &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 1.0e-3, 1.0e-3, -1.0e-3],
        &[
            1.0e10, 0.0, 0.0, 0.0, 1.0e10, 0.0, 1.0e-10, 1.0e-10, 1.0e-10,
        ],
        &[3.0, 0.7, 0.1, 0.3, 5.0, 0.2, 0.37, 0.91, 1.7],
    ];
    for (i, m) in persp.iter().enumerate() {
        geometry_registers_with(
            c,
            &format!("matrix_perspective/m{i}"),
            &[StageSpec::with(Op::MatrixPerspective, f32s(m))],
            Precision::BOTH,
            &[],
            true,
        );
    }
}

/// `repeat_x/y`, `mirror_x/y` (highp only, `TileCtx`) and `{clamp,repeat,mirror}_x_1`.
fn tiles(c: &mut Cases) {
    let scales: &[f32] = &[1.0, 2.0, 3.0, 7.5, 16.0, 100.0, 255.0, 0.25, 1000.5];
    for (i, &scale) in scales.iter().enumerate() {
        let inv_scale = 1.0f32 / scale;
        for (name, op) in [("repeat_x", Op::RepeatX), ("repeat_y", Op::RepeatY)] {
            geometry_registers(
                c,
                &format!("{name}/s{i}"),
                &[StageSpec::with(
                    op,
                    Ctx::Tile {
                        scale,
                        inv_scale,
                        mirror_bias_dir: -1,
                    },
                )],
                Precision::HIGHP,
                &[scale],
            );
        }
        for (name, op) in [("mirror_x", Op::MirrorX), ("mirror_y", Op::MirrorY)] {
            for dir in [-1, 1] {
                geometry_registers(
                    c,
                    // `trunc_` of a negative `s` (R5): Scalar differs from the x64 proxy.
                    &format!("{name}/r5/s{i}/dir{dir}"),
                    &[StageSpec::with(
                        op,
                        Ctx::Tile {
                            scale,
                            inv_scale,
                            mirror_bias_dir: dir,
                        },
                    )],
                    Precision::HIGHP,
                    &[scale],
                );
            }
        }
    }
    for op in [Op::ClampX1, Op::RepeatX1, Op::MirrorX1] {
        geometry_registers(
            c,
            op.name(),
            &[StageSpec::new(op)],
            Precision::BOTH,
            &[1.0, 2.0],
        );
    }
}

/// `clamp_x_and_y`.
fn clamps(c: &mut Cases) {
    let variants: &[[f32; 4]] = &[
        [0.0, 0.0, 100.0, 50.0],
        [0.0, 0.0, 1.0, 1.0],
        [-5.0, -5.0, 5.0, 5.0],
        // Max below min.
        [10.0, 10.0, -10.0, -10.0],
        [
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
            f32::INFINITY,
            f32::INFINITY,
        ],
        [0.5, 0.25, 255.5, 1000.75],
        [-0.0, 0.0, 0.0, -0.0],
    ];
    for (i, &v) in variants.iter().enumerate() {
        let scale = (v[2] - v[0]).abs().clamp(1.0, 1000.0);
        // v6 has ±0 bounds: `fminf(+0, -0)` differs between wasm's musl and the x64 proxy (R5).
        let r5 = if i == 6 { "r5/" } else { "" };
        geometry_registers(
            c,
            &format!("clamp_x_and_y/{r5}v{i}"),
            &[StageSpec::with(Op::ClampXAndY, Ctx::CoordClamp(v))],
            Precision::BOTH,
            &[scale],
        );
    }
}

/// `decal_x`, `decal_y`, `decal_x_and_y` followed by `check_decal_mask` (which masks `r, g, b, a`
/// with what the decal stage stored in their shared context).
fn decals(c: &mut Cases) {
    // (limit_x, limit_y, inclusive edge x, inclusive edge y): the edges are 0 or the limit
    // (`roundDownAtInteger`).
    let variants: &[([f32; 2], [f32; 2])] = &[
        ([100.0, 50.0], [0.0, 0.0]),
        ([100.0, 50.0], [100.0, 50.0]),
        ([1.0, 1.0], [0.0, 0.0]),
        ([1.0, 1.0], [1.0, 1.0]),
        ([255.5, 7.25], [0.0, 7.25]),
        ([0.0, 0.0], [0.0, 0.0]),
        ([1.0e9, 3.0], [1.0e9, 0.0]),
    ];
    for (i, &(limit, edge)) in variants.iter().enumerate() {
        let scale = limit[0].clamp(1.0, 1000.0);
        for (name, op) in [
            ("decal_x", Op::DecalX),
            ("decal_y", Op::DecalY),
            ("decal_x_and_y", Op::DecalXAndY),
        ] {
            let ctx = Ctx::Decal { id: 1, limit, edge };
            geometry_registers(
                c,
                &format!("{name}/v{i}"),
                &[
                    StageSpec::with(op, ctx.clone()),
                    StageSpec::with(Op::CheckDecalMask, ctx),
                ],
                Precision::BOTH,
                &[scale, limit[1].clamp(1.0, 1000.0)],
            );
        }
    }
}

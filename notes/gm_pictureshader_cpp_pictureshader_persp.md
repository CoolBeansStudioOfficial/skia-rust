# gm_pictureshader_cpp_pictureshader_persp

`pictureshader_persp` mismatches in **f16 only** (8888 and 565 match).

## Root cause (found, third attempt): host libm `tanf`, glibc vs UCRT

`SkM44::Perspective(0.01f, 10.f, SK_ScalarPI / 3.f)` computes `cot = 1 / std::tan(halfAngle)`
with `halfAngle = 0x3f060a92` (0.52359879f). The exact tangent lies 0.4998 ulp above
`0x3f13cd3a`, almost half way to `0x3f13cd3b`:

| libm | `tanf(0x3f060a92)` | `cot` |
|---|---|---|
| exact (correctly rounded) | `0x3f13cd3a` | `0x3fddb3d8` |
| UCRT (oracle, Windows) | `0x3f13cd3a` | `0x3fddb3d8` |
| glibc 2.39 (this host; Rust's `f32::tan` calls it) | `0x3f13cd3b` | `0x3fddb3d6` |

- The UCRT value comes from the UCRT-exact `tanf` on branch `port/libm` (PR #87,
  `crates/skia-rust-core/src/libm/trig_f32.rs`, exhaustively checked against `ucrtbase.dll`):
  for this argument it is `tan_poly((double)x)` rounded to float, evaluated by hand (not by
  merging that branch).
- The glibc value was checked twice: `std::tan(float)` in a C++ program, and Rust's `f32::tan`.
  (The previous note said glibc was correctly rounded here; it is not, by 0.0002 ulp.)
- Real Skia's own arithmetic for the whole matrix chain (`SkM44.cpp`, `SkMatrix.cpp`,
  `SkMatrixInvert.cpp`, `SkPoint.cpp`, `SkRect.cpp` compiled with clang on this host:
  `M44` ops, canvas `preTranslate`/`preConcat`, `setGlobalCTM`, `asM33`, the picture shader's
  `DifferentialAreaScale` tile size (99 x 99), `lm`, `MatrixRec::apply`'s concat and invert) gives
  the same `matrix_perspective` context as our port, bit for bit, *when both use glibc's tan*:
  `3f12540e 0 c27b8078 3caa456b 3f1252dc c11a4ecf bb8997c5 0 3faf7c4e`. So the port of the chain is
  exact; only the libm input differs from the oracle's.
- Confirmation: with `tanf`'s result lowered by one ulp (the UCRT value) in `M44::perspective`
  (a local hack, not committed), the whole GM passes: every config, every tier (scalar, sse2,
  sse41, ml3, ml4). Of the 27 combinations of {-1, 0, +1} ulp on `tan`, `sin(0.008)` and
  `cos(0.008)`, only `tan - 1` passes (with `cos` 0 or +1; `sin` and `cos` from glibc are the
  UCRT's here).

Why only f16 shows it: the cot difference moves the sample coordinates of the bilinear F16 tile
by ~1e-7 relative. 8888 and 565 quantize the result to 8/6 bits and hide it; f16 shows 1..4
half-ulps at the glyph edges, where texels change fastest. The positions differ per tier because
`matrix_perspective` and the bilinear stages round differently per tier (FMA on ml3/ml4), on top
of the same input difference.

## Status

Blocked on libm: per the project rule, a mismatch that traces to host libm vs the oracle's UCRT
is recorded and left. It should pass once a UCRT-exact `tanf` is used by `scalar_tan`/
`M44::perspective` (`port/libm`, on hold for a licensing decision). It would also fail on macOS
if Apple's `tanf` rounds this argument up.

The oracle N32 emulation (picture shader tile `kRGBA_8888` not N32 in the BGRA oracle) does not
affect this GM: the f16 tile is `kRGBA_F16Norm`, and a full `gm-verify` gives the same hashes with
and without it.

## Earlier attempts (superseded by the above)

- Perturbing each entry of the final `matrix_perspective` context by +-1 or +-2 ulps singly never
  reached 0 differences: the libm difference changes several entries at once.
- Making the tile `RGBA_F32` instead gives 410 differing pixels, so the golden tile really is
  F16-quantised.
- Differing pixels per golden tier class: scalar 24, sse2..avx 27, ml3 11, ml4 9; only G differs
  (green text over black), by 1..4 half ulps.

# dm/lottie/skottie-gradient-radial-highlight.json (and skottie-bulge.json)

## skottie-gradient-radial-highlight.json
Status: `todo`. Mismatches on sse2 and sse41 only (scalar-model ml3 and native ml4 match).

- The animated highlight `h` runs 0 -> 100 -> -100 -> 0 percent with the angle rotating; `Gradient.cpp` pins `h_len` to `[-1 + eps, 1 - eps]`, so the focal point sits almost exactly on the end circle (the `xy_to_2pt_conical_focal_on_circle` neighbourhood: huge `t`, near-zero divisors).
- `oracle/rp-diff` (docs/PORTING.md section 12) has no two-point-conical case (`grep -i "conical\|focal" oracle/rp-diff/src/cases.rs` is empty), and the oracle host no longer exists, so no new case could be run. The ported `ConicalGradientsGM` / `shallowgradient conical` GMs pass on every tier, so the stages are right for ordinary inputs; the difference needs the near-degenerate range.
- Suspects (sse2/sse41 differ from ml3/ml4/scalar): float to int conversion of huge `t` (`cvttps2dq` overflow gives 0x80000000, where the portable and AVX2 paths may saturate), `floor_` emulation on SSE2, and min/max NaN operand order. Suggested rp-diff cases when an oracle build is available: `xy_to_2pt_conical_{focal_on_circle,well_behaved,greater,smaller}` + `mask_2pt_conical_{nan,degenerates}` + `alter_2pt_conical_*` at inputs with `|t|` around 1e6..1e9 and focal x = 1 - eps.
- Hashes (sse2): ours `8f2e62eb...` golden `bd9f107b...`.

## skottie-bulge.json
Re-checked with the clean-room libm (sinf/cosf/tanf/atanf) merged: still mismatches on every tier (ml4 ours `7cde3f22...` golden `abfcec2c...`).
The effect (`BulgeEffect.cpp`) uses on the host side `std::pow(adjHeight, 3)` (double `pow`) and `std::asin(1/r)` (float `asinf`); the shader itself only uses the raster pipeline's own `asin_`/`approx_powf` (pure arithmetic). The clean-room libm has no `asinf` or `pow`; the port calls the host's. Experiment: `asinf` replaced by the correctly rounded `(f64::asin as f32)` changes the hash (so `asinf` does matter: glibc `asinf` differs from the correctly rounded value on 348 of 4000 sampled inputs) but still does not match, so the oracle's UCRT `asinf` and/or `pow` rounding differ from both. Needs a clean-room `asinf` and `pow` matching UCRT, which cannot be checked without it.

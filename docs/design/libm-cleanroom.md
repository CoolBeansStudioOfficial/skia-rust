# skia-rust-libm: clean-room provenance log

Goal: `skia_rust_core::libm` (crate `skia-rust-libm`) returns bit-identical results to the Windows x64
UCRT (the CPU path every modern CPU takes) for the functions Skia's ported code calls.

## Clean-room rules followed

* No look at branch `port/libm`, PR #87, `docs/design/math.md`, or any `notes/*` file about
  libm/arcto/parsedpaths/longpathdash. No disassembly or reading of `ucrtbase.dll`/`msvcrt`/Microsoft
  CRT sources, no third-party reverse-engineered copy.
* Allowed inputs only: textbook/public algorithms (Maclaurin series, Payne-Hanek reduction, the
  classic five-interval atan reduction of fdlibm `s_atan.c`, Pade-style rational approximation) and
  **black-box observation**: calling the real UCRT functions (through Rust `std` on
  `windows-latest`, which links `ucrtbase`) on inputs we choose and recording outputs.
* The probe is `crates/skia-rust-libm/examples/probe.rs`, driven by
  `.github/workflows/libm-cleanroom-probe.yml`. Modes: `jobs` (record platform outputs for chosen
  inputs), `check` (compare the crate against the platform on every f32 bit pattern), `ledger`
  (near-rounding-boundary inputs, using the platform's own f64 function as a ~1e-9-ulp reference),
  `near` (our pre-rounding values close to the platform's rounding boundary), `nsel`.
* Offline analysis (Python/mpmath/numpy/scipy) lives outside the repo; it only ever sees platform
  outputs, never platform code.

## Method that worked

1. Dump platform outputs on stratified inputs, compare with the correctly rounded value (mpmath).
   UCRT float functions are *not* correctly rounded: they are double-precision evaluations of short
   approximations rounded once to float (error 1e-4..1e-2 float ulps), which is why host libms differ.
2. Guess a textbook structure, test it against the dumps (0 mismatches required).
3. Where the polynomial/rational is unknown, fit it: every platform result gives an interval
   (the rounding interval of the returned float) that the pre-rounding double must lie in; near
   a rounding boundary these are tight. A Gauss-Newton linear program (maximise the margin) over
   those intervals recovers coefficients; the feasible set only exists if the *shape* is right, so
   infeasibility rules shapes out. Mismatches found by the exhaustive check are fed back as
   further constraints (counterexample-guided).
4. Verify exhaustively over all 2^32 float bit patterns on the Windows runner.

## sinf / cosf  (exact: 0 mismatches of 2^32)

Started from: Maclaurin series; Payne-Hanek reduction (public); Horner with FMA.
Measurements that fixed each choice:
* `|x| <= pi/4`: degree-9 sine / degree-10 cosine Maclaurin in f64 matched 0/33,028 samples
  (degree 7/8 and 11+ did not); the platform error vs truth (<= 0.027 float ulp) equals the
  truncation error of those series.
* `|x| < 2^-7`: `x - x^3/6` and `1 - x^2/2` (the full series mismatched 14 / 97 inputs, only below 2^-7).
* Reduction: using an exactly rounded `r = x - n*pi/2` gave 0 mismatches for 1e3..1e30 where a
  single-constant Cody-Waite did not; two residual inputs (+-123650.85 sinf; +-5.88e8 cosf) were
  explained by (a) FMA Horner and (b) `n = floor(|x|*2/pi + 0.5)` computed in double (quadrant can
  be off by one next to pi/4, so |r| may slightly exceed pi/4).
* `nsel` probe (per binade): the double-precision quadrant is used up to somewhere in
  (5.9e8, 3.6e9]; beyond, exact. Chosen limit 2^31 (no float between the observed points differs).
* NaN: signalling NaN is quieted (bit 22 set); infinities give the default NaN 0xFFC00000; -0.0 kept.
* Exhaustive result: sinf, cosf: 0 mismatches.

## tanf  (exact: 0 mismatches of 2^32)

Structure: `x*P(x^2)/Q(x^2)` for `|x| <= pi/4`, same reduction, `-1/tan(r)` for odd quadrants.
Fits: polynomial in x^2 up to degree 9 infeasible; sin/cos Taylor ratio mismatched; rational
(4,2) feasible but degenerate; the exhaustive check plus counterexample constraints then showed
(2,2) with numerator constant 1 is feasible with a larger margin, i.e. 4 free coefficients.
Coefficients are fitted, not published. Final: 0 mismatches (run 38202467663).

## atanf

Error pattern (lobes with a gap at 0.45..0.65 and a second regime above 2.4) showed fdlibm's five
intervals (7/16, 11/16, 19/16, 39/16 edges, atan(0.5), pi/4, atan(1.5), pi/2 offsets). Core
`w*P(w^2)/Q(w^2)`: (2,2), (4,1), (5,0) infeasible; (3,2) feasible and reproduces all 5e5 ledger
entries in every interval. Status: see "Results" below.

## Other functions

See "Results".

## Observation from the CI-hosted oracle workflow (coordinator, admissible black-box evidence)

Run https://github.com/CoolBeansStudioOfficial/skia-rust/actions/runs/38197457719: a Linux build of
Skia rendering GMs with an LD_PRELOAD shim that computes the float libm functions as
`(float)f((double)x)` with glibc's double functions made these GMs byte-identical to the Windows
goldens: parsedpaths, shadow_utils, shadow_utils_occl, gradients_powerless_hue_LCH,
imagefilter_transformed_image. longpathdash (double sin/cos) dropped from 15 to 5 differing pixels;
gradients_color_space_many_stops / gradients_powerless_hue_OKLCH (cbrtf), blurrect_compare and
poster_circle were not fixed. This only concerns the inputs those GMs use, not exhaustive equality;
our own UCRT probes remain the authority. It is consistent with our finding that the float
functions are double evaluations rounded once.

## Results

(filled in at the end)

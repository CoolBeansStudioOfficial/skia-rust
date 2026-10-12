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

## atanf  (exact: 0 mismatches of 2^32)

Error pattern (lobes with a gap at 0.45..0.65 and a second regime above 2.4) showed fdlibm's five
intervals (7/16, 11/16, 19/16, 39/16 edges, atan(0.5), pi/4, atan(1.5), pi/2 offsets). Core
`w*P(w^2)/Q(w^2)` fitted on the exhaustive near-midpoint ledger (5e5 entries, x in [2^-14, 2^27]):
(2,2), (4,1), (5,0) infeasible; (3,2) feasible and reproduces every ledger entry in every interval.
The coefficients are one point of a degenerate feasible set (numerator-minus-denominator
coefficient of w^2 is -1/3, as it must be); the exhaustive Windows check is the evidence.

## atan2f  (NOT exact: 1326 mismatches of 4e8 random operand pairs)

* The quotient is formed in double precision (float quotient: thousands of mismatches).
* Reduction: the same five intervals as atanf, `PI - a` for negative x, sign of y applied last.
* Small quotients: `atan(q) = q` (no correction) when `q < 1e-4` or the biased exponent of `y`
  is at least 14 below that of `x` (these two conditions together; dense data, kappa window of
  the fit `p = q(1 + kappa q^2)` is [-0.0003, 0.0001] in both groups, and 0 mismatches over 2e6 samples).
* Specials (784 pairs of zeros, infinities, NaNs, extremes): all match; NaN operand order: x wins.
* Residual: 1326 of 4e8 random pairs (3.3e-6), all with `0.05 <= q <= 0.43` or `2.5 <= q <= 3.2`
  (the core region). atanf's core is not the same function as atan2f's: a joint fit is infeasible
  (best margin -2.7e-4 half-ulps), and so is a (3,2)/(4,2)/(3,3)/(4,3) core fitted to the atan2f data
  alone (15-30 contradictory points remain, i.e. the pre-rounding value is not a smooth function of
  `y/x` there; float-rounded quotients fit some of them). Per-group cores gave 1416 mismatches, no
  better. Two serious attempts made; callers keep using the module (3e-6 mismatch rate beats the
  host libm) but the function is flagged non-exact, and the Windows test bounds the rate.

## Functions not reproduced (measurements only)

CR = correctly rounded (mpmath, 200+ bits). "excess" = how far past the rounding boundary the true
value lies when the platform result differs from CR (a lower bound on the platform's pre-rounding
error, in result ulps).

| function | platform differs from CR | max excess | what the data says |
|---|---|---|---|
| double `sin`, `cos` | 1.2% / 1.2% | 0.25 ulp | Not fdlibm (0.9% mismatch vs platform), not a Maclaurin series in Horner/Estrin/split form with or without FMA (best 2% mismatch on [0.1, pi/4]); table-driven or extended-precision structure suspected. Two attempts. |
| `cbrtf` | 30% | 1.26 ulp | mean error 0 in every mantissa bucket, std 0.43..0.53 ulp, independent of mantissa: float-arithmetic noise, not a smooth approximation; exponent mod 3 changes the noise level. Operation sequence not identified. |
| `cbrt` (double) | 13% | 0.9996 ulp | similar, double arithmetic. |
| `asinf` / `acosf` | 1.2% / 0.34% | 0.32 / 0.14 ulp | noisy, float-arithmetic-like. |
| `expf` | 7.5e-5 | 4.8e-4 ulp | error always one-sided (platform below truth) and not a smooth function of x: a polynomial in r = x - k ln2 of any degree 6..12 (round or floor reduction) and a (5,5) rational in r are all infeasible (best margin -4.4e-4 and -5.6e-4 half-ulps), and truncating the exact value to 26..49 bits does not explain it either; |x| < 2^-7 also has its own short form. |
| `logf`, `log2f` | 1e-4 (3.8e-3 near 1) | 0.04 / 0.25 ulp | informative points dense with random signs (66,000 in [0.5, 2]): float-arithmetic noise, not LP-fittable. |
| `powf` | 4e-4 | n/a | also returns 0 for results that would round to the smallest denormal. |
| double `acos`, `exp`, `exp2`, `log`, `pow` | 2.7%, 0.5%, 2e-4, 3e-5, 3.9e-4 | 0.39, 0.91, 0.003, 0.003, 0.0015 ulp | `log`, `exp2`, `pow` are close to correctly rounded; matching would need the exact error terms. |

Callers of these functions stay on the platform function. The clippy guard only covers the
functions that are exact (`f32::sin/cos/tan/sin_cos`).

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

Final probe run (Windows Server runner, `windows-latest`):
https://github.com/CoolBeansStudioOfficial/skia-rust/actions/runs/38224425216

| function | mismatches |
|---|---|
| `sinf`, `cosf`, `tanf`, `atanf` | 0 of 4,294,967,296 bit patterns each (NaN payloads, infinities, zeros, subnormals included) |
| `atan2f` | 1326 of 4e8 random pairs; 0 of 784 special pairs |
| everything else | not implemented (see above) |

Caveat: the probe runner's `ucrtbase.dll` is whatever `windows-latest` ships; the oracle used
Windows 11. The functions agree with the oracle on every GM that exercises them (below).

GMs verified byte-identical to the goldens on every tier after routing `scalar_sin/cos/tan/atan2`
and `M44::Perspective` through the module: `parsedpaths`, `shadow_utils`, `shadow_utils_occl`,
`shadow_utils_gray`, `pictureshader_persp`, `imagefilter_transformed_image`. Still failing for
reasons outside this module: `longpathdash` (double sin/cos), `gradients_*` (cbrtf, atan2f noise),
`shadow_utils_directional` (f16 ml tiers), `p3_ovals`, `skbug_14554`, `emboss`.

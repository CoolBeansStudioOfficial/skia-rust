# Math library: UCRT-exact transcendental functions

Skia calls the C library for `sinf`, `cosf`, `atan2f`, `pow`, … (`SkScalarSin` is `(float)std::sin(x)`,
`SkCubics` calls `std::acos`/`std::cbrt`, GMs call `sin`/`cos`/`exp` directly). None of these is
correctly rounded in any common C library, so their last bits depend on the host. The goldens were
rendered on the oracle host (real Skia m156 built with clang-cl on Windows 11 x64), whose C library is
the **Universal CRT** (`ucrtbase.dll`). A one-ulp difference in a conic control point or a line
endpoint moves anti-aliased coverage by one, which is how the host dependence showed up:

| GM | Symptom before | Cause |
|---|---|---|
| `gm/arcto.cpp::parsedpaths` | 47 px ±1 on Linux | `atan2f`/`tanf`/`cosf` in SVG arc conversion (`SkPathBuilder::arcTo`) |
| `gm/dashing.cpp::longpathdash` | 5 px on every host | `sin(a)` with a float `a` is `sinf` under MSVC's `<cmath>` (it was ported as the double function), plus double `sin`/`cos` |
| `gm/arcto.cpp::arcto` | matched on Linux and Windows, not on macOS arm64 | Apple's `sinf`/`cosf`/`tanf` round differently |

`skia_rust_core::libm` replaces every ported libm call. It is pure safe Rust (IEEE `+ - * /`, `sqrt`,
correctly rounded `mul_add`, integer bit manipulation and constant tables), so it computes the same
bits on every host, and it reproduces the UCRT's x64 results bit for bit.

## What the UCRT does

Findings from probing the UCRT on GitHub's `windows-latest` runners (ucrtbase 10.0.26100) and from the
x64 code of `ucrtbase.dll`:

* **Nothing is correctly rounded.** Exhaustively over all 2^32 inputs, the UCRT's float functions differ
  from `(float)f((double)x)` (which is almost always the correctly rounded result) on 2,866,998 inputs
  for `sinf`, 2,876,136 for `cosf`, 480,626 `tanf`, 2,352,772 `asinf`, 693,829 `acosf`, 3,532 `atanf`,
  23,280 `expf`, 210,637 `logf`, 313,550 `log2f`; checked against a 200-bit reference (mpmath), the
  UCRT is the one that is off, by one or a few ulps. The double functions are faithful but not
  correctly rounded either (on 20,000 random inputs: 535 `sin`, 525 `cos`, 213 `acos`, 96 `exp`,
  12 `log`, 11 `pow` and 5,751 `cbrt` results differ from the correctly rounded value). So a
  correctly rounded (CORE-MATH style) library would *not* match the goldens; the UCRT's algorithms have
  to be reproduced.
* **Two code paths.** Each x64 math function checks a process-wide flag (`_set_FMA3_enable`, on by
  default when the CPU has FMA3/AVX2) and runs either an FMA3 or an SSE2 implementation; their results
  differ (e.g. `sinf` mismatch counts change when the flag is cleared). Every Windows 11 machine has
  FMA3, and the oracle's goldens match the FMA3 path (the three GMs above pass with it), so the module
  implements the **FMA3 path** only.
* **Algorithms.** The float trigonometric functions evaluate in double: reduction by π/2 (Cody–Waite
  with a 33-bit π/2 head below ~2^24 or ~2^31, an integer Payne–Hanek reduction against a 2/π bit
  table above), then a short Taylor polynomial (`sin` to x⁹, `cos` to x¹⁰) or a rational `tan`
  approximation, rounded to float. `atan2f` uses a 241-entry table of `atan(k/256)` and a cubic
  correction; `atanf` fdlibm's reduction in double; `asinf`/`acosf`/`acos` fdlibm's algorithms with
  fused multiply-adds; `expf`/`exp` a `2^(j/64)` table (`exp` with head/tail entries and a truncating
  index); `logf` a single-precision 129-entry table; `log` a 257-entry head/tail table; `powf`
  `exp(y·log x)` in double, whose near-1 branch finishes with the SSE2 `exp` kernel; `log2f`, `exp2`
  and `pow` the algorithms of ARM's optimized-routines (as in glibc) with the UCRT's own operation
  order; `cbrt` a rational first guess refined by two Newton steps (`cbrtf` the same in single precision with one). `sin`/`cos` are fdlibm-style
  kernels on a head/tail reduced argument.

The module was written from the x64 code of `ucrtbase.dll` 10.0.26100: control flow, constants and
tables are taken from it, the operation order (including which multiply-adds are fused) mirrors it,
and each function carries a `// Port of: UCRT …` line.

## Verification

* A temporary probe on `windows-latest` (PR for this change) compared every function with the real
  UCRT: **exhaustively over all 2^32 inputs** for `sinf`, `cosf`, `tanf`, `asinf`, `acosf`, `atanf`,
  `expf`, `logf`, `log2f`, `cbrtf`; 2^28 random argument pairs for `atan2f` and `powf`; 2^26 inputs each for
  `sin`, `cos`, `acos`, `cbrt`, `exp`, `log`, `exp2` and `pow`. Zero mismatches (NaN payloads are not
  compared; the module returns the UCRT's quiet-NaN conventions, but no test depends on payloads).
* `crates/skia-rust-core/tests/libm_ucrt.rs` keeps this in CI: on Windows x64 it compares every function
  against the platform functions (which are the UCRT) on ~10^5 inputs in debug and ~5·10^6 in release,
  plus every argument the three GMs above pass (`tests/data/libm_gm_args.txt`). On every host it checks
  a digest of the same results, so a host where the module computes anything differently (e.g. a broken
  `mul_add`) fails as well.

## Inventory (chrome/m156 call sites)

| Function | Skia callers ported so far |
|---|---|
| `sinf`, `cosf` | `SkScalarSin/Cos(SnapToZero)`: `SkPathBuilder` arcs (`arcTo`, `addArc`, SVG arcs), `SkMatrix::setRotate`, `SkM44::setRotate`, `SkGeometry` cubic roots; GMs (`addarc`, `aaa`, `pathfill`, `dashing`, `hairlines`, `sharedcorners`, `convex_all_line_paths`, `batchedconvexpaths`, `largeclippedpath`, `tool_utils`) |
| `tanf` | `SkPathBuilder::arcTo` (SVG), `SkM44::Perspective` |
| `atan2f` | `SkPathBuilder::arcTo` (SVG) |
| `acosf` | `SkMeasureAngleBetweenVectors`, `SkGeometry` cubic roots |
| `powf` | `SkScalarPow`: cube roots in `SkGeometry` and `SkCubicMap`; `SkPixmap` sRGB decode; `SkColorSpaceXformSteps` HDR OOTF |
| `asinf`, `atanf`, `expf`, `logf`, `log2f` | `SkScalar*` wrappers, `crbug_1041204` GM, unit tests |
| `acos`, `cos`, `cbrt` | `SkCubics::RootsReal` |
| `cbrtf` | (for the OKLCH gradient conversion, `SkGradientBaseShader`, not yet ported) |
| `sin`, `cos` | `longpathdash` GM |
| `exp`, `log`, `pow`, `exp2` | unit tests (`RandomTest`, `RTreeTest`, `QuadRootsTest`, `FloatingPointTest`, `SkRasterPipelineOpts`) |

Not libm, and ported as Skia code: skcms's `powf_`/`log2f_`/`exp2f_` approximations, the raster
pipeline's and SkSL's `approx_*`/`sin_`/`atan_` kernels (`skia_rust_simd`), `sk_float_*` rounding
helpers. Exact in IEEE and left on `f32`/`f64`: `sqrt`, `floor`, `ceil`, `trunc`, `round`, `abs`,
`copysign`, `%`, and `mul_add` where Skia calls `fma`. (`powi`, used for compile-time-like constants
and by the `SkParse` float reader, is integer repeated multiplication in compiler-builtins on every
host.)

## Rules

* Ported code calls `skia_rust_core::libm`, never `f32::sin` & co.; `clippy.toml` lists them under
  `disallowed-methods` with the replacement in the message. The only allowed users are tests that
  compare against the platform library on purpose.
* Choose the function the C++ resolves to on MSVC: a `float` argument to `sin`, `std::sin`, `exp`, …
  is the float function (MSVC's `<cmath>` declares the float overloads globally), a `double` argument
  the double one.
* A function the module lacks is added by porting it from the UCRT's x64 FMA3 code the same way, with a
  `// Port of: UCRT …` line, a table emitted from the DLL where it has one, a column in
  `libm_ucrt.rs`, and an updated digest. Don't add a `std` fallback.
* Errno, floating-point exception flags and NaN payloads are not modelled.

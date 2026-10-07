# Design: Raster Pipeline and SIMD tiers (Phase 2, CPU raster)

Status: proposed (2026-10-06). Pin: `chrome/m156` (`95ee33d7ec77`). Audience: every agent working on
`skia-rust-simd`, the Raster Pipeline, blitters, scan converters and the Phase 2 GM sweep.

All Skia references are `path#Lx-Ly` in the pinned tree (`third_party/skia`). "RP" = Skia's
Raster Pipeline, "opts" = `src/opts/SkRasterPipeline_opts.h`.

## Decisions at a glance

1. **Six tiers, grounded in Skia's code paths, not in ISA names:** `Scalar`, `Sse2`, `Sse41`, `Ml3`,
   `Ml4`, `Neon` (§2.2). The 19 x64 oracle tiers collapse into exactly the four x86 variants; the
   wasm oracle is Skia's `SKRP_CPU_SCALAR` path (Skia never vectorizes RP on wasm).
2. **What actually differs** is a short, closed list of primitives (§1.3): float `min`/`max`, `mad`
   (fused or not), `floor`/`ceil`, `rcp`/`rsqrt` estimates, float→int conversions, `pack`,
   half-float conversion, mask tests, integer division, lowp `div255`, plus the lane counts. All
   other RP code is lane-wise IEEE/integer arithmetic and is written once, portably.
3. **The tier-visible differences in the m156 goldens are accounted for** (by inspection of which
   GMs differ; A5 confirms it stage by stage) by three of those items: the `rcp_fast` estimate (sse2 → sse41, 52 outputs), fused `mad` + F16C (→ ml3, 1,020
   outputs), and `rcp14` vs `rcpps` (ml3 → ml4, 104 outputs) (§1.9).
4. **Hardware estimates are vendor-specific.** On this Zen 4 host `rcpps`/`rsqrtps` are exact
   functions of the top 12 mantissa bits (4096-entry tables model them bit-exactly); `rcp14`/
   `rsqrt14` follow Intel's published reference algorithm bit for bit. All four have software
   models exact on all 2³² inputs (`estimates::amd_zen4`, A2d). Goldens embed the oracle host's
   estimate tables (§1.4, §4.6).
5. **Stage code is written once and stamped per tier** with `macro_rules!` + `include!`, inside
   per-tier modules whose every function is a safe `#[target_feature]` function. Generic code
   over a `Lanes` trait was rejected: trait methods cannot carry `#[target_feature]` and generic
   callers cannot call intrinsics without `unsafe` (verified with rustc 1.99, §2.4).
6. **Execution is a per-tier `match` interpreter over a typed program** (`Stage<'a>` enum with
   borrowed contexts), not a tail-call chain: stable Rust has no guaranteed tail calls, and a
   `match` loop keeps the eight registers in real registers and inlines stage bodies (§2.6).
7. **Tail handling replicates Skia's MemoryCtx patching exactly**, including scratch buffers whose
   stale lanes persist across rows and across `run()` calls of a compiled pipeline (§1.7).
8. **`unsafe` is limited to two kinds**: calling a tier's entry point after detection, and the
   array↔register conversions (`loadu`/`storeu` on `&[T; N]`). About 70 blocks in total; all
   pixel memory access, gathers and lane math are safe Rust (§3).
9. **Every tier has a model twin**: the same stage source stamped with per-lane reference
   primitives (no target features). Models run on any host (and under Miri), so CI can check
   every tier's goldens everywhere; native/model equality is the scalar-twin test (§2.8, §4).
10. **An oracle-side per-stage harness (`oracle/rp-diff`) and a DM stage-list dump** are Wave A
    deliverables; GM mismatches are debugged stage by stage, never by trial and error (§4).

---

## 1. What differs between tiers in Skia

### 1.1 Where Skia chooses code

Skia picks CPU code in two places:

- **Compile time.** `SkRasterPipeline_opts.h` selects one of `SKRP_CPU_SCALAR`, `SKRP_CPU_NEON`,
  `SKRP_CPU_ML4`, `SKRP_CPU_AVX2`, `SKRP_CPU_AVX`, `SKRP_CPU_SSE41`, `SKRP_CPU_SSE2` (or LoongArch)
  from `SK_CPU_X64_LEVEL` / `SK_ARM_HAS_NEON` (`opts#L76-L100`). The level comes from compiler
  macros (`include/private/SkFeatures.h#L108-L159`). `SkOpts.cpp` compiles the header once at the
  library baseline into the default tables `ops_highp`/`ops_lowp` (`src/core/SkOpts.cpp#L25-L44`).
- **Run time.** `SkOpts::Init()` replaces the tables with `ml3` or `ml4` kernels when `SkCpu`
  reports `ML3 = AVX2|BMI1|BMI2|F16C|FMA` or `ML4 = AVX512F|DQ|CD|BW|VL`
  (`src/core/SkOpts.cpp#L51-L72`, `src/core/SkCpu.h#L41-L43`, `src/opts/SkOpts_ml3.cpp`,
  `src/opts/SkOpts_ml4.cpp`). ml4 requires `SK_ENABLE_AVX512_OPTS`, which GN defines
  unconditionally (`BUILD.gn#L131-L132`). ml3 is compiled with `-march=x86-64-v3` (`/arch:AVX2`
  on Windows), ml4 with `-march=x86-64-v4 -mprefer-vector-width=512` (`BUILD.gn#L177-L202`).

Consequences:

- **There is no runtime SSE4.1 dispatch.** A default (SSE2-baseline) Skia on an SSE4.1 CPU without
  AVX2 runs the `Sse2` code. `Sse41` only exists for builds whose *baseline* is SSE4.1/4.2/AVX.
- **An AVX baseline runs the SSE4.1 code** (`SKRP_CPU_AVX` shares the 4-wide block with
  `SKRP_CPU_SSE41`, `opts#L933-L1156`), which is why `cpu-x64-avx` ≡ `cpu-x64-sse41`.
- **The `ssse3` runtime level only touches** `SkBitmapProcState_opts`, `SkBlitMask_opts` and
  `SkSwizzler_opts` (`src/core/SkBitmapProcState_opts.cpp#L30-L33`,
  `src/core/SkBlitMask_opts.cpp#L28-L31`); it measured byte-identical to sse2 over the suite.
- **wasm32 runs `SKRP_CPU_SCALAR`.** Emscripten defines `__clang__` but no x86 level and no NEON,
  so the `#elif` chain at `opts#L76-L100` ends in `SKRP_CPU_SCALAR`, and the lowp pipeline is
  compiled out (`opts#L5465-L5480`). `-msimd128` only changes `SkVx`'s `any`/`all`
  (`src/core/SkVx.h#L562-L607`), which are exact; CanvasKit's own build does not even pass it
  (`modules/canvaskit/compile.sh#L206-L240`). **So the wasm tier is a 1-lane, highp-only,
  scalar-semantics tier.** (PLAN §5.1/§6.3 should be updated accordingly; the name
  `WasmSimd128` is replaced by `Scalar`.)
- On the Windows oracle (clang-cl defines `_MSC_VER`) stages use the "narrow" ABI
  (`opts#L1754-L1773`): `dr..da` live in a `Params` struct instead of registers. This has no effect
  on results.

### 1.2 Lane counts

| Tier | highp `N` (`F` width) | lowp `N` (`U16` lanes) | Source |
|---|---|---|---|
| Scalar | 1 (`float`) | — (no lowp; every pipeline is highp) | `opts#L124-L131`, `opts#L5465-L5480` |
| Sse2 / Sse41 | 4 (`__m128`) | 8 | `opts#L933-L940`, `opts#L5484-L5488` |
| Ml3 | 8 (`__m256`) | 16 | `opts#L688-L696`, `opts#L5484-L5488` |
| Ml4 | 16 (`__m512`) | 16 | `opts#L334-L341`, `opts#L5484-L5488` |
| Neon (arm64) | 4 | 8 | `opts#L205-L212`, `opts#L5484-L5488` |

`N = sizeof(F)/sizeof(float)` (`opts#L1748`); lowp `N = sizeof(U16)/2` (`opts#L5499`). The stride
is observable only through tail lanes (§1.7), `seed_shader`/`dither` iotas (`opts#L2300-L2345`)
and SkSL `any`/`all` branches (`opts#L4336-L4350`); lane-wise arithmetic does not depend on it.
We still use Skia's widths: it keeps stale-lane behaviour identical and it is what performance
parity needs.

### 1.3 Primitive semantics per tier (highp)

This is the list our implementation must reproduce. Everything not in this table (`+ - * /`,
comparisons, bit ops, shifts, integer `min`/`max`/`abs`, `sqrt` on the targeted tiers, gathers,
interleaved loads/stores) is lane-wise exact and identical on every tier.

| Primitive | Scalar | Sse2 | Sse41 | Ml3 | Ml4 | Neon (arm64) |
|---|---|---|---|---|---|---|
| `min(F,F)` / `max(F,F)` | `fminf`/`fmaxf` (NaN ignored) `#L133-L138` | `minps`/`maxps`: `a<b?a:b` / `a>b?a:b`, NaN or ±0 tie → **second operand** `#L950-L951` | same | `vminps`/`vmaxps` `#L701-L706` | `vminps zmm` `#L345-L350` | `vminq`/`vmaxq` = `FMIN`/`FMAX`: NaN **propagates**, −0 < +0 `#L215-L220` |
| `mad(f,m,a)` / `nmad` | `a+f*m`, unfused `#L140-L141` | unfused `#L968-L969` | unfused | **fused** `_mm256_fmadd_ps`/`fnmadd` `#L698-L699` | **fused** `#L343-L344` | **fused** `vfmaq`/`vfmsq` `#L238-L239` |
| `abs_(F)` | `fabsf` | `v & (0-v)` `#L970`: NaN keeps its sign bit | same | same `#L708` | same `#L351` | `vabsq` (clears sign) |
| `floor_`/`ceil_` | `floorf`/`ceilf` | **emulated** `cvt(cvtt(v))` ∓ 1 `#L1065-L1081`: `floor(-0.0)=+0.0`, NaN or magnitude ≥ 2³¹ → `-2147483648.0` | `roundps` (exact) | `vroundps` | `vrndscaleps` | `vrndmq`/`vrndpq` |
| `rcp_approx` / `rsqrt_approx` | `1/v`, `1/sqrtf(v)` (exact) | `rcpps` / `rsqrtps` | same | `vrcpps`/`vrsqrtps` `#L712-L713` | **`vrcp14ps`/`vrsqrt14ps`** `#L355-L356` | `vrecpe`+1 NR step / `vrsqrte`+1 step `#L224-L226` |
| `rcp_precise` | `1/v` | `e*(2-v*e)`, `e=rcpps` `#L977` | same | `fnmadd(v,e,2)*e`, `e=vrcpps` `#L715-L718` | same with `rcp14` `#L358-L361` | 2 NR steps `#L225` |
| `rcp_fast` / `rsqrt` | `rcp_precise` / `rcp_precise(sqrt)` | **`rcp_precise` / `rcp_precise(sqrt)`** `#L1737-L1741` | `rcp_approx` / `rsqrt_approx` `#L1742-L1745` | `rcp_approx` / `rsqrt_approx` | same | same |
| `iround` / `round` | `(int)(v+0.5f)` (trunc after +0.5) `#L151-L152` | `cvtps2dq`: ties-to-even, NaN/overflow → `0x80000000` `#L1042-L1043` | same | same `#L720-L721` | same `#L362-L363` | `vcvtnq`: ties-to-even, saturating, NaN → 0 `#L243-L244` |
| `trunc_`, `cast`, any `F→I32` vector cast | C cast | `cvttps2dq` (NaN/overflow → `0x80000000`) `#L1595-L1599` | same | same | same | `FCVTZS` (saturating, NaN → 0) |
| `pack(U32→U16)` | truncation `#L153` | **truncation** (sign-extend + `packssdw`) `#L1049-L1051` | **saturation** `packusdw` `#L1047` | saturation `#L722-L725` | saturation `#L364-L368` | truncation (`convertvector`) `#L228` |
| `pack(U16→U8)` | truncation | `packuswb` (signed-saturating) `#L1055-L1059` | same | same `#L726-L729` | same `#L369-L372` | truncation `#L229` |
| `if_then_else(c,t,e)` | `c ? t : e` (nonzero) `#L156-L157` | bitwise `and/andnot/or` `#L942-L948` | same | `blendvps`: **sign bit only** `#L731-L734` | **sign bit only** (`and 0x80000000`, `test`) `#L373-L382` | `vbslq` bitwise `#L231-L232` |
| `any`/`all` | `c != 0` `#L159-L160` | `movmskps`: lane sign bits only `#L1062-L1063` | same | `vptest` over the whole register: any bit set / every bit set `#L737-L738` | `vptestmd`: some / every lane has any bit set `#L383-L390` | `vmaxvq`/`vminvq`: some / every lane nonzero `#L235-L236` |
| comparison result | **0/1** (`bool`); `cond_to_mask` converts `#L2277-L2286` | all-ones mask | same | same | same | same |
| `from_half`/`to_half` | software: denorm halfs flushed to 0, `to_half` **truncates** mantissa `#L1661-L1694` | software | software | **F16C** `vcvtph2ps`/`vcvtps2ph` (RNE, denormals kept) `#L1658-L1682` | F16C zmm `#L1655-L1679` | `vcvt_f32_f16`/`vcvt_f16_f32` (RNE) `#L1652-L1676` |
| `div_fn(I32)` (SkSL `/`) | generic: `x/0` → `x/-1`, `INT_MIN/-1` guarded `#L4950-L4961` | via `f64`, `cvttpd`: `x/0` → `INT_MIN` `#L985-L1003` | same | same `#L780-L803` | same `#L439-L488` | generic |
| `div_fn(U32)` | generic: `x/0` → `x/0xFFFFFFFF` `#L4963-L4970` | via `f64` after **clamping both operands to `INT_MAX`** `#L1016-L1023` | same (`pminud`) `#L1012-L1015` | same `#L813-L836` | **exact** `vcvtudq2pd`/`vcvttpd2udq` `#L491-L540` | generic |

Notes that matter in practice:

- **Stores round differently on Scalar.** `to_unorm` is `round(min(max(0, mad(v,scale,bias)), maxI))`
  (`opts#L2269-L2272`). x86/NEON round ties to even; Scalar computes `(U32)(v+0.5f)`. `v*255 = 2.5`
  stores 2 on x86/NEON and 3 on Scalar. NaN stores `maxI` on x86 (`maxps(0,NaN)=NaN`,
  `minps(NaN,maxI)=maxI`) but 0 on NEON and Scalar.
- **`floor_` on Sse2** feeds `fract`, tiling (`repeat_x_1`, `mirror_x_1`, `opts#L3604-L3605`) and
  `mod`. Large or non-finite coordinates give garbage on Sse2 only.
- **The Scalar tier's masks are 0/1.** Code that stores comparison results must go through
  `cond_to_mask` exactly where Skia does.
- **FMA is explicit.** Only `mad`/`nmad` (and `rcp_precise` on Ml3/Ml4/Neon, `vrecps`/`vrsqrts` on
  Neon) are fused. The oracle builds with `-ffp-contract=off` (`oracle/README.md`, Findings), and
  rustc never contracts, so `a*b + c` written in C++ stays unfused in Rust.
- `sqrt_` is exact on all targeted tiers; arm32's estimate-based `sqrt_` (`opts#L262-L267`) is out
  of scope (we target aarch64 only).

### 1.4 Approximations (estimates) and where they are used

Users of `rcp_fast`, `rcp_precise` and `rsqrt` in highp:

| Stage | Line | Primitive |
|---|---|---|
| `colorburn`, `colordodge` | `opts#L2474-L2483` | `rcp_fast` |
| `set_sat`, `clip_color` (hue/saturation/color/luminosity) | `opts#L2521-L2556` | `rcp_fast` |
| `matrix_perspective` | `opts#L3721-L3728` | `rcp_precise` |
| SkSL `invsqrt_*` | `opts#L4771`, `opts#L4797` | `rsqrt` |
| SkSL `inverse_mat2/3/4` | `opts#L4817-L4866` | `rcp_precise` |

lowp: `matrix_perspective` (`opts#L6131-L6138`) uses lowp `rcp_precise`, which on x86/NEON splits
into two highp `rcp_precise` calls (`opts#L5939-L5966`); Ml4 uses `rcp14` directly.

`rcp_precise` is **not** exact: one Newton–Raphson step from a 12-bit estimate does not always
round to `1/x`. Measured on this host over all 2²³ mantissas in [1,2): the Sse2/Sse41 form matches
`1/x` for 5,080,721 inputs (61%), the Ml3 fused form for 6,091,541 (73%), the Ml4 `rcp14` form for
6,569,701 (78%). So even the "precise" Sse2 tier depends on the estimate table.

**Measured on the oracle host (AMD Ryzen 7 7800X3D, Zen 4)** with a throwaway probe
(rustc 1.99, `core::arch`):

- `rcpss` ≡ `rcpps` lane 0 ≡ `vrcpps` (ymm) on all 2²³ mantissas: the estimate is per-lane and
  width-independent.
- `rcpps` and `rsqrtps` results are exact functions of the **top 12 mantissa bits** (not of 11),
  and `rcpps(x·2ᵉ) = rcpps(x)·2⁻ᵉ` in the normal range. A 4096-entry table (8192 for `rsqrtps`,
  exponent parity) models them bit-exactly. Specials (A2d, verified over all 2³² inputs):
  NaN → the input quieted (`| 0x400000`, sign and payload kept); ±0 **and every denormal
  input** → ±∞ (denormals read as zero, sign kept, also for `rsqrtps(-denormal) = -∞`); `+∞` →
  `+0`, `rcpps(-∞) = -0`; `rsqrtps` of any other negative input (normal or `-∞`) → the default
  NaN `0xffc00000`; `rcpps` results below the normal range (`|x| ≥ 2¹²⁶`) flush to ±0 (no
  denormal results). `rsqrtps` results are always normal.
- `vrcp14ps`/`vrsqrt14ps` are Intel's reference algorithm (§6 R2): `rcp14` depends on the top
  16 mantissa bits, `rsqrt14` on the top 15, plus an exact power-of-two case (`rcp14(2ᵉ) = 2⁻ᵉ`,
  `rsqrt14(4ᵉ) = 2⁻ᵉ`). (A1's "all 23 bits" came from that special case at mantissa 0.) Unlike
  the 12-bit forms they handle denormals: denormal inputs are normalized (`rcp14(x)` = ±∞ only
  for `|x| ≤ 0x00200000`, where `1/x` overflows), results down to `2⁻¹⁴⁹` are produced as
  denormals, and `rsqrt14(-denormal)` is the default NaN.
- Fingerprints (FNV-1a of the outputs over [1,2), [2,4) for `rsqrt`) for comparison with other
  hosts: `rcpps[1,2)=f7ba415b37cf2325`, `rsqrtps[1,2)=df1c5fd75f2c2325`,
  `rsqrtps[2,4)=a918bea842f82325`, `rcp14[1,2)=80dd67e92b89c525`, `rsqrt14[1,2)=9c830943fb7e9e25`.

The SDM only bounds `rcpps`/`rsqrtps` error (|rel err| ≤ 1.5·2⁻¹²); Intel and AMD implement
different tables, so **Sse2/Sse41/Ml3 outputs that use any estimate are vendor-specific**. Ml4's
are expected to be portable across vendors: Zen 4's `rcp14`/`rsqrt14` follow Intel's reference
algorithm and coefficient tables exactly (§6 R2; still worth confirming on one Intel AVX-512
runner). ARM's `FRECPE`/`FRSQRTE` are defined
bit-exactly by Arm ARM pseudocode (`RecipEstimate`/`RecipSqrtEstimate`), and `FRECPS`/`FRSQRTS`
are fused, so Neon outputs are portable across Apple and Neoverse cores.

Other approximations (`approx_log2`, `approx_pow2`, `approx_powf`, polynomial `sin_`/`cos_`/`tan_`/
`atan_`, `opts#L1610-L1649`, `opts#L2099-L2240`) are plain arithmetic: they differ between tiers
only through `mad` (fused or not), `floor_`, `round` and `min`/`max`. They are not vendor-specific.

### 1.5 Half floats

`load_f16`/`store_f16` (and every `*f16` stage) use `from_half`/`to_half`. On Scalar, Sse2 and Sse41
these are bit tricks that flush half denormals (including inputs that become half denormals) to 0
and **truncate** the mantissa (`opts#L1661-L1694`); on Ml3/Ml4/Neon they are IEEE conversions with
round-to-nearest-even (F16C with `_MM_FROUND_CUR_DIRECTION`, default MXCSR). This, together with
fused `mad`, is why 795 of 909 `f16` outputs differ between sse2 and ml3.

### 1.6 lowp differences

| Primitive | x86 (all four) | Neon | Source |
|---|---|---|---|
| `div255(v)` (used by most blend modes, `lerp`) | `(v+255)/256` (off by one ~25% of the time) | exact `vrshrq(vrsraq(v,v,8),8)` | `opts#L5701-L5714` |
| `div255_accurate` (srcin/out, srcover/dstover, modulate, screen) | `(v+128 + ((v+128)>>8))>>8` | same as `div255` (exact) | `opts#L5716-L5728`, `opts#L6255-L6268` |
| `min`/`max` (all types) | compare-select `x<y ? y : x` | same | `opts#L5771-L5806` |
| `max_intr`/`min_intr` on `F` | `maxps`/`minps` per 128/256/512-bit half | compare-select | `opts#L5813-L5889` |
| `floor_` | Sse2: emulated (`cast(cast<I32>)`); Sse41+: `roundps` | `vrndmq` | `opts#L6008-L6035` |
| `scaled_mult` (Q15 `mulhrs`) | Sse41+: `pmulhrsw`; Sse2: generic `(a*b + 2¹⁴) >> 15` (same results) | `vqrdmulh`: **saturates** `-32768*-32768` to 32767 | `opts#L6042-L6063` |
| `mad` | unfused on every tier | unfused | `opts#L5920-L5934` |
| `rcp_precise` | two highp calls (Ml4: `rcp14`) | two highp calls | `opts#L5939-L5966` |
| 8888 load/store | shuffles/packs (exact) | `vld4`/`vst4` (exact) | `opts#L6452-L6570` |
| gradient lookup ≤ 8 stops | Ml3: `vpermps`; others gather (same values) | gather | `opts#L3730-L3799` |

Integer lowp arithmetic is wrapping 16-bit (`ext_vector_type` semantics). `SK_USE_INACCURATE_DIV255_IN_BLEND`
is not defined in GN builds (`opts#L6239`).

Pipeline choice: `buildLowpPipeline` succeeds only if every op has a lowp implementation on the
tier, there is no `stack_rewind`, and `gForceHighPrecisionRasterPipeline` is unset
(`src/core/SkRasterPipeline.cpp#L588-L604`); otherwise highp (`#L606-L632`). On Scalar every lowp
entry is null (`opts#L5473-L5476`), so **Scalar always runs highp**, including the simple 8888
fills that every SIMD tier runs in lowp.

### 1.7 Tail handling (identical on all tiers, but must be replicated exactly)

There are no masked loads/stores. `start_pipeline` runs full `N`-pixel chunks, then one tail chunk
with `*tailPointer = tail` (`opts#L1787-L1828`, lowp `opts#L5533-L5570`). For the tail, every
`MemoryCtx` the pipeline registered (`SkRasterPipeline.cpp#L67-L160`, `addMemoryContext` `#L695-L712`)
is **patched** to point into a per-context scratch buffer (`kMaxScratchPerPatch` bytes,
`src/core/SkRasterPipelineOpContexts.h#L27-L68`): loads copy `tail*bpp` bytes in, stores copy
`tail*bpp` bytes back (`opts#L1697-L1735`).

Observable details we must copy:

- Scratch buffers are zeroed once per `run()` (`SkRasterPipeline.cpp#L656-L661`) but **once per
  `compile()`** (`#L679-L685`): a compiled pipeline (used by blitters for every `blitH`) keeps the
  previous tail's bytes in lanes `tail..N`. Lanes beyond the tail therefore hold deterministic
  stale data, which only becomes visible through cross-lane operations (SkSL `any`/`all`), but we
  replicate it rather than argue it away.
- Gathers and SkSL slot memory are not patched; they read clamped indices or full-width buffers.
- `branch_if_all_lanes_active` reads the tail (`opts#L4336-L4342`); the tail value is `0xFF` outside
  the tail chunk.

### 1.8 Other `SkOpts` kernels

| Kernel | x86 tiers | Neon | Scalar (wasm) | Source |
|---|---|---|---|---|
| `blit_row_s32a_opaque` | `s + (d*(256-a))>>8` with saturating add; AVX2 8-wide + SSE2 4-wide + scalar tail, all the same formula | `SkMulDiv255Round(255-a, d)` (**exact rounding, different results**) | scalar `SkPMSrcOver` (same as x86 for premul input) | `src/opts/SkBlitRow_opts.h#L22-L117`, `#L164-L237` |
| `blit_row_color32` | `SkVx` (portable, exact) | same | same | `SkBlitRow_opts.h#L243-L276` |
| `blit_mask_d32_a8` | portable `Sk4px` code | NEON code | portable | `src/opts/SkBlitMask_opts.h#L20-L414` |
| `S32_alpha_D32_filter_DX(DY)` (legacy image sampling) | SSE2 (`#L183`) and SSSE3 (`#L44`) variants | NEON | portable | `src/opts/SkBitmapProcState_opts.h` |
| `memset16/32/64`, `rect_memset*` | exact | exact | exact | `src/opts/SkMemset_opts.h` |
| swizzlers (codecs, Phase 4) | ssse3/ml3 variants | NEON | portable | `src/opts/SkSwizzler_opts.inc` |

Within x86 these kernels are formula-identical across levels (only the SSE2 vs SSSE3 bitmap
sampler is a separate implementation, see R9), and the oracle shows no difference between x86
levels from them. On Neon, `blit_row_s32a_opaque` and `blit_mask_d32_a8` are different formulas.
For premultiplied input the portable/scalar `SkPMSrcOver` equals the x86 formula; with
non-premultiplied garbage the x86 saturating add and the scalar wrap-around differ.

### 1.9 The four measured x86 behaviours, explained

From the published goldens (`goldens-m156`, 2,727 results per tier; computed for this document):

- 1,701 results (62%) are identical across all four behaviours.
- **sse2 → sse41: 52 outputs**, all blend-mode or vertex/patch-blend GMs (`xfermodes*`,
  `aaxfermodes`, `HSL_duck`, `hsl`, `lcdblendmodes`, `colorcomposefilter_wacky`,
  `coloremoji_blendmodes_test`, `dstreadshuffle`, `ducky_yuv_blend`, `patch_*`, `vertices*`,
  `xfermodeimagefilter`, `draw-atlas-colors`, `lcdoverlap`). This is `rcp_fast` switching from
  `rcp_precise` to the raw estimate (`opts#L1737-L1745`) in colorburn/colordodge and the HSL modes.
  Perspective GMs are *not* in this set because both tiers use the same `rcp_precise`.
  (The Sse2 `floor_`/`pack` emulations produced no visible difference in this suite.)
- **sse41 → ml3: 1,020 outputs** (795 `f16`, 125 `8888`, 100 `565`): fused `mad` in every highp
  stage (gradients, color filters, color-space transforms, matrices) and F16C conversions.
- **ml3 → ml4: 104 outputs**: exactly the perspective GMs (`persp_*`, `*_perspective*`,
  `filltypespersp`, `localmatrixshader_persp`, `clip_shader_persp`, …) and the blend-mode GMs:
  the `rcp14`/`rsqrt14` estimates.
- Compile-time builds equal their runtime counterparts (`v3` ≡ any `*-rt-ml3`, `v4` ≡ any
  `*-rt-ml4`, `sse41` ≡ `avx`): **code outside `SkOpts` is tier-invariant** on the whole suite
  (it is compiled at the baseline, and `SkVx`'s SIMD paths are exact).

### 1.10 Implementer checklist

When porting any stage or kernel:

1. Port the arithmetic with the portable lane operators; call a tier primitive (§2.4) for every
   operation in §1.3/§1.6 — never `f32::min`, `as i32`, `round()` or `mul_add` directly.
2. Keep Skia's operand order in `min`/`max`/`if_then_else` (x86 returns the second operand on
   NaN; the order is part of the result).
3. Keep `mad` where Skia writes `mad`, and `a + b*c` where Skia writes that.
4. Keep `cond_to_mask` where Skia has it (Scalar masks are 0/1).
5. Where Skia has `#if defined(SKRP_CPU_…)` around a stage body (e.g. `gradient_lookup`,
   `from_8888`, `seed_shader` on LSX), check whether the branches are semantically identical; if
   not, the variation belongs in the tier's lane module, not in the stage.

---

## 2. Rust architecture

### 2.1 Crate placement

| Skia | Rust |
|---|---|
| `src/opts/SkRasterPipeline_opts.h`, `src/core/SkOpts.*`, `SkRasterPipelineOpList.h`, `SkRasterPipelineOpContexts.h` | `skia_rust_simd::raster_pipeline` (`rp`): ops, contexts, lane types, tier primitives, stages, interpreter |
| `src/opts/SkBlitRow_opts.h`, `SkBlitMask_opts.h`, `SkMemset_opts.h` (later `SkBitmapProcState_opts.h`, `SkSwizzler_opts.inc`) | `skia_rust_simd::{blit_row, blit_mask, memset, …}` |
| `src/core/SkCpu.*` | `skia_rust_simd::cpu` (features, tokens); `skia_rust_simd::tier` (`Tier::detect`) |
| `src/core/SkRasterPipeline.{h,cpp}` (builder: `append*`, `buildLowpPipeline`, `compile`, `run`, `dump`) | `skia_rust_core::raster_pipeline` (needs `ColorType`, `Matrix`, skcms) |
| blitters, scan converters, edges, `SkRasterClip`, `SkAAClip`, `SkDraw`, `SkBitmapDevice`, raster `Surface` | `skia-rust-raster` (new crate) |
| `SkStroke`, `SkStrokeRec`, `SkPaint`, `SkCanvas` + `Device` trait, `SkClipStack`, `SkRecord*`/`SkPicture` | `skia-rust-core` |
| `SkDashPath`, `SkDashPathEffect` | `skia-rust-effects` (`src/effects` → effects, PORTING §2) |

`SkRasterPipeline_opts.h` lives under `src/opts`, which PORTING §2 already maps to `skia-rust-simd`;
putting the stages there is also what keeps the `#[target_feature]` call graph — and therefore the
`unsafe` — inside one crate (§3). `Canvas` in core talks to a `Device` trait; `skia-rust-raster`
implements `BitmapDevice` and the raster `Surface` constructors, and the facade re-exports them at
`skia-safe`'s paths (record in `docs/API_MAPPING.md`).

### 2.2 The tier enum

`crates/skia-rust-simd/src/tier.rs` (committed with this design) replaces the old ISA-named
variants:

```rust
#[non_exhaustive]
pub enum Tier { Scalar, Sse2, Sse41, Ml3, Ml4, Neon }
```

| `Tier` | Skia path | Oracle tiers it must match |
|---|---|---|
| `Scalar` | `SKRP_CPU_SCALAR` | `wasm-simd128` (planned; proxy: an `x64-scalar` build, §4.5) |
| `Sse2` | `SKRP_CPU_SSE2` | `cpu-x64-sse2`, `cpu-x64-ssse3`, `cpu-x64-sse2-rt-ssse3` |
| `Sse41` | `SKRP_CPU_SSE41`/`AVX` | `cpu-x64-sse41`, `cpu-x64-sse42`, `cpu-x64-avx` |
| `Ml3` | `SKRP_CPU_AVX2` | `cpu-x64-{sse2,ssse3,sse41,sse42,avx}-rt-ml3`, `cpu-x64-v3` |
| `Ml4` | `SKRP_CPU_ML4` | `cpu-x64-*-rt-ml4`, `cpu-x64-v3-rt-ml4`, `cpu-x64-v4` |
| `Neon` | `SKRP_CPU_NEON` + arm64 | `arm64-neon` (planned) |

The rest of the API (implemented in task A1):

```rust
impl Tier {
    pub const ALL: [Tier; 6];
    /// What real Skia built like this crate would run here: compile-time baseline
    /// (`cfg!(target_feature = "sse4.1")` → Sse41, else Sse2; aarch64 → Neon; wasm32 → Scalar),
    /// upgraded at run time to Ml3 / Ml4 exactly like `SkOpts::Init` (never to Sse41).
    pub fn detect() -> Tier;                 // cached in a OnceLock (an immutable constant)
    pub fn is_native(self) -> bool;          // can this host execute the tier's instructions?
    pub const fn highp_stride(self) -> usize;           // 1, 4, 4, 8, 16, 4
    pub const fn lowp_stride(self) -> Option<usize>;    // None, 8, 8, 16, 16, 8
    pub fn oracle_tiers(self) -> &'static [&'static str];
}

/// How a tier is executed.
pub enum Backend {
    /// The tier's real instructions (requires `is_native()`).
    Native,
    /// The tier's model twin (§2.8): per-lane reference primitives, any host, Miri-compatible.
    Model(Estimates),
}
/// Source of `rcp`/`rsqrt` estimates in a model.
pub enum Estimates { Host, AmdZen4, /* IntelCore: once measured */ Arm }

#[derive(Clone, Copy)]
pub struct Selection { pub tier: Tier, pub backend: Backend }
pub fn selection() -> Selection;             // forced (testing) or detect() + Native
```

Testing hook (feature `testing`, never in release builds of downstream crates):

```rust
pub mod testing {
    /// Forces the tier for the current thread until the guard drops. Fails if `Backend::Native`
    /// is requested for a tier the host cannot run.
    pub fn force_tier(sel: Selection) -> Result<TierGuard, Unsupported>;
    pub struct TierGuard { /* restores the previous selection; !Send */ }
}
```

The override is thread-local (each GM render runs on one thread) and is read where Skia reads its
`SkOpts` tables: when a pipeline is compiled and when a blitter picks `blit_row`/`memset` kernels.
This is the only mutable state, it is test-only, and it is scoped.

**As implemented in A1** (decisions where the sketch above was open):

- **Modules.** `skia_rust_simd::cpu` is the `SkCpu` port: `CpuFeatures` (bitflags with `SkX64`'s
  bit values, incl. `ML3`/`ML4`), `CpuFeatures::read()` (`read_cpu_features()`, decoding `cpuid`
  exactly like Skia), `X64Level` (`SK_CPU_X64_LEVEL`, from `cfg!(target_feature)` as
  `SkFeatures.h` derives it), `CpuFeatures::supports` (`SkCpu::Supports`, ORing in the baseline),
  `CpuCap`, and the tokens. `skia_rust_simd::tier` holds `Tier`, `select_x64` (pure:
  baseline + features → tier, i.e. `SKRP_CPU_*` choice + `SkOpts::Init`), `Selection`, `Backend`,
  `Estimates`, `Unsupported`, `selection()`. `skia_rust_simd::estimates` runs the host's estimate
  instructions (§1.4). `skia_rust_simd::testing` (feature `testing`, also on under `cfg(test)`).
- **Skia's decoding, not `std`'s.** `detect()` uses the ported `cpuid` decoding, because it differs
  from `is_x86_feature_detected!`: Skia reports AVX-512 only on AMD or on Intel with VBMI2 (Ice
  Lake+), so a Skylake-X runs `Ml3` in Skia and must in skia-rust too. `std` detection builds the
  tokens. If Skia's decoding ever claims a tier whose `#[target_feature]` list `std` does not fully
  confirm, `detect()` steps down (ml4 → ml3 → compile-time path); `detect()` is always native.
- **Tokens.** `Sse2Token`, `Sse41Token`, `Ml3Token`, `Ml4Token`, `NeonToken`: zero-sized,
  `get() -> Option<Self>` (cached `OnceLock<bool>`), `FEATURES` = the exact string the tier's
  `#[target_feature(enable = …)]` must use (§2.4 table). `Sse2Token`/`NeonToken` exist for
  uniformity of the §3.1 safety argument even though their features are baseline.
- **Compile-time baseline.** Rust's `x86_64-pc-windows-msvc` target enables `sse3` by default, so
  `X64Level::compiled()` is `Sse3` there; Skia built with `-msse3` also runs `SKRP_CPU_SSE2`, so
  the tier is still `Sse2`. An SSE1-only x86 build would be `Scalar`.
- **CPU cap.** `CpuCap {Baseline, Ssse3, Ml3, Ml4}` reproduces `SKIA_ORACLE_CPU_CAP`
  (`oracle/patches/skia-oracle.patch`) bit for bit and is exposed as `Tier::detect_with_cap(cap)`
  (uncached; `detect()` = `detect_with_cap(Ml4)`). The library does **not** read an environment
  variable: production detection must be what Skia does, and tests choose tiers with
  `force_tier`. A unit test runs `select_x64` for all 19 x64 oracle tiers (build level + cap on a
  Zen 4 feature set) and checks each lands on the `Tier` whose `oracle_tiers()` lists it; an xtask
  test checks `oracle_tiers()` covers exactly the tiers derived from `oracle/tiers.toml`.
- **`oracle_tiers()`** lists the canonical tier first (`cpu-x64-sse2`, `cpu-x64-sse41`,
  `cpu-x64-sse2-rt-ml3`, `cpu-x64-sse2-rt-ml4`: what a default Skia runs). `Scalar` lists
  `wasm-simd128` then the `cpu-x64-scalar` proxy; harnesses use the first with published goldens.
  `Tier::oracle_tiers_rgba()` lists the one RGBA-variant tier per class (§4.4, N32 byte order);
  `Tier::for_oracle_tier(name)` is the inverse of both.
- **`Selection::check()`** (also run by `force_tier`): `Native` needs `is_native()`;
  `Model(AmdZen4)` only for x86 tiers, `Model(Arm)` only for `Neon`, `Model(Host)` only if the
  host has the tier's estimate instructions (`rcpps` for Sse2/Sse41/Ml3, `vrcp14ps` for Ml4,
  NEON for Neon). `Scalar` uses no estimates and accepts every backend.
- **`force_tier`** pushes onto a thread-local stack; `TierGuard` (`!Send`, `#[must_use]`) pops on
  drop and panics if guards are dropped out of order. `selection()` consults the stack only under
  `cfg(any(test, feature = "testing"))`; without the feature it is `detect()` + `Native`, with no
  thread-local access. Parallel tests cannot interfere (tested with overlapping threads).
- **Estimate fingerprints** (`estimates::Fingerprints`, `AMD_ZEN4`): FNV-1a over 32-bit words,
  `h = (h ^ bits) * 0x100000001b3` from `0xcbf29ce484222325`, over the outputs for inputs
  `base | m`, `m = 0..2²³` ascending (`base` = `0x3f800000` for [1,2), `0x40000000` for [2,4)).
  This reproduces the §1.4 values on the oracle host. `Fingerprints::matches_for(reference, tier)`
  implements the §4.6 per-tier policy check.
- **`cargo xtask cpu-probe [--dump-tables DIR]`** prints vendor/brand, `SkCpu` features, the
  compile-time level, `detect()` and `detect_with_cap` for every cap, the native tiers, the five
  fingerprints with a verdict against `AMD_ZEN4`, and a per-tier "estimates match the goldens'
  host" line. `--dump-tables` writes `rcpps.bin` (4096 × u32 LE, entry `i` = output for
  `0x3f800000 | i << 11`), `rsqrtps.bin` (8192 × u32 LE: [1,2) then [2,4)) and `estimates.txt`
  (host, fingerprints, mismatch counts), and checks the tables against the host over all 2²³
  mantissas of [1,2) (`rcpps`) and [1,4) (`rsqrtps`): 0 mismatches on the oracle host. Specials
  and other binades are covered by A2d's exhaustive check (§2.8).

### 2.3 Dispatch

Production: `Tier::detect()` once; every pipeline/kernel call matches on the tier and calls that
tier's entry point. Entry points are safe `#[target_feature]` functions; calling one from the
non-feature dispatcher is the one place `unsafe` is required (§3.1):

```rust
pub(crate) fn run_highp(sel: Selection, prog: &Program<'_>, rect: Rect4, mem: &mut MemoryBindings<'_>) {
    match (sel.tier, sel.backend) {
        #[cfg(target_arch = "x86_64")]
        (Tier::Ml3, Backend::Native) => {
            let tok = Ml3Token::get().expect("selection checked is_native");
            // SAFETY: `tok` exists only if avx2,fma,f16c,bmi1,bmi2 were detected at run time.
            unsafe { tiers::ml3::highp::run(tok, prog, rect, mem) }
        }
        (t, Backend::Model(e)) => tiers::model::run_highp(t, e, prog, rect, mem),
        // …
    }
}
```

`Ml3Token`/`Ml4Token`/`Sse41Token` are zero-sized proofs built only from
`is_x86_feature_detected!` (cached in `OnceLock`). `Sse2` and `Neon` are baseline on their
targets (`cfg(target_feature = "sse2")`, `cfg(target_feature = "neon")`), but rustc still requires
`unsafe` to call a `#[target_feature(enable = "sse2")]` function from a function that does not
itself carry the attribute — verified on 1.99: *"the sse target feature being enabled in the build
configuration does not remove the requirement to list it in `#[target_feature]`"*.

### 2.4 Lane types and tier primitives

**Why not `trait Lanes` + generic stages.** Checked against rustc 1.99:

- `#[target_feature]` on a safe trait method is rejected (*"cannot be applied to safe trait
  method"*), so a trait impl cannot be a feature context.
- A generic (non-`#[target_feature]`) function cannot call a `#[target_feature]` function or an
  intrinsic without `unsafe` (E0133), even when it will be inlined into a feature context.
- `#[inline(always)]` together with `#[target_feature]` is not stable.

A generic design therefore needs one `unsafe` per intrinsic call per tier (~55 primitives × 5
SIMD tiers ≈ 275 blocks) whose safety rests on a type invariant. The stamped design below needs
none of those.

**Lane types** are plain arrays. For the SIMD tiers and their models we reuse the already-ported
`skia_rust_simd::vx::Vec<N, T>` (`#[repr(transparent)]` over `[T; N]`): its operators already
have clang `ext_vector_type` semantics — the same semantics `SkRasterPipeline_opts.h` uses
(`opts#L44-L53`) — IEEE for floats, wrapping for integers, comparisons returning all-ones masks.
Task A2a marks its operators, comparisons and `bit_cast` helpers `#[inline(always)]` so they are
guaranteed to inline into feature-enabled callers. The `Scalar` tier instead uses a one-lane
wrapper `S<T>` with the same method names but C scalar semantics: **comparisons yield 0/1**
(`bool`), exactly what `cond_to_mask` (`opts#L2277-L2286`) exists to fix up. Because stage code
is stamped rather than generic (§2.5), tiers may use different lane types as long as the names
match.

```rust
// vx::Vec<N, T>: + - * / neg, & | ^ !, << >> (by scalar), eq_mask/lt_mask/… -> Vec<N, Mask>,
// cast::<D>(), plus (added in A2a) bit_cast::<U>() between same-size lane vectors.
pub struct S<T>(pub T);   // Scalar tier: same API, comparisons -> S<i32> holding 0 or 1
```

`#[inline(always)]` portable operators are inlined into the feature-enabled stage functions and
auto-vectorized there. Checked: a stage computing `mad(dr, 1 - a, r)` and `mad(...) * rcp_approx(b)`
on an 8-lane `f32` array type in an `avx2,fma` function compiles to `vbroadcastss`, `vsubps`, two
`vfmadd213ps`, `vrcpps`, `vmulps` and stores — no array round trips — and LLVM performs no unsafe
float rewrites.

**Tier lane modules** (`rp/lanes/{scalar,sse2,sse41,ml3,ml4,neon}.rs`) each export the same names;
this list is the complete per-tier semantic surface (from §1.3/§1.6):

```rust
pub const N: usize;            // highp stride
pub const LOWP_N: usize;       // lowp stride (unused on Scalar)
pub type F = Vec<N, f32>;  pub type I32 = Vec<N, i32>;  pub type U32 = Vec<N, u32>;   // Scalar: S<f32>, …
pub type U16 = Vec<N, u16>; pub type U8 = Vec<N, u8>;   pub type U64 = Vec<N, u64>;
// highp primitives, each `si!`-wrapped (= #[target_feature(enable = TIER_FEATURES)] #[inline]):
fn min_f(a: F, b: F) -> F;              fn max_f(a: F, b: F) -> F;
fn mad(f: F, m: F, a: F) -> F;          fn nmad(f: F, m: F, a: F) -> F;
fn abs_f(v: F) -> F;
fn floor_(v: F) -> F;                   fn ceil_(v: F) -> F;
fn rcp_approx(v: F) -> F;               fn rsqrt_approx(v: F) -> F;
fn rcp_precise(v: F) -> F;              fn rcp_fast(v: F) -> F;   fn rsqrt(v: F) -> F;
fn iround(v: F) -> I32;                 fn round(v: F) -> U32;
fn trunc_(v: F) -> U32;                 fn to_i32(v: F) -> I32;   // every F→int vector cast
fn pack_u32(v: U32) -> U16;             fn pack_u16(v: U16) -> U8;
fn if_then_else_f(c: I32, t: F, e: F) -> F;  fn if_then_else_i(c: I32, t: I32, e: I32) -> I32;
fn any(c: I32) -> bool;                 fn all(c: I32) -> bool;
fn cond_to_mask(c: I32) -> I32;         // identity except Scalar
fn from_half(h: U16) -> F;              fn to_half(f: F) -> U16;
fn div_i32(d: I32, s: I32) -> I32;      fn div_u32(d: U32, s: U32) -> U32;
// lowp primitives (module `lowp`, LOWP_N lanes):
fn div255(v: U16) -> U16;  fn div255_accurate(v: U16) -> U16;
fn min_intr_f / max_intr_f;  fn floor_(v: LF) -> LF;  fn rcp_precise(v: LF) -> LF;
fn scaled_mult(a: I16, b: I16) -> I16;
// conversions used by the primitives (x86/NEON only), the only `unsafe` in the module (§3.2):
fn to_reg(v: F) -> __m256;  fn from_reg(r: __m256) -> F;   // etc. per register type
```

Overloads become suffixed names (`min(F,F)` → `min_f`, `min(F, float)` → `min_f(a, F::splat(b))`).
Integer `min`/`max`/`abs`, `sqrt_`, gathers and `load2/load4/store2/store4` give identical results
on every tier and are portable lane loops on `V`, not primitives.

Rust feature strings (Skia's in `src/opts/SkOpts_SetTarget.h#L74-L131`, `BUILD.gn#L177-L202`):

| Tier | `#[target_feature(enable = …)]` |
|---|---|
| Sse2 | `"sse2"` |
| Sse41 | `"sse2,ssse3,sse4.1"` |
| Ml3 | `"sse2,ssse3,sse4.1,sse4.2,avx,avx2,bmi1,bmi2,f16c,fma"` |
| Ml4 | Ml3's + `"avx512f,avx512dq,avx512cd,avx512bw,avx512vl"` |
| Neon | `"neon"` |
| Scalar, models | none |

**As implemented in A2a** (`crates/skia-rust-simd/src/rp/lanes/`; the module docs of
`rp::lanes` list every exported name with its per-tier semantics):

- **Modules.** `rp::lanes::{scalar, sse2, sse41}` (the last two on `x86_64` only), the models
  `model_sse2::host` and `model_sse41::host` (feature `models`, always on in this crate's tests),
  `x86_model` (per-lane models of the x86 instructions: SSE NaN propagation, the QNaN indefinite
  `0xFFC00000`, the integer indefinite `0x80000000`, `minps`/`maxps`, `cvt(t)ps2dq`, `cvttpd2dq`,
  `packssdw`/`packusdw`/`packuswb`, `movmskps`, `pmulhrsw`, `roundps`, `rcpps`/`rsqrtps` from an
  `Estimates`), `portable` (functions identical on every SIMD tier: integer `min`/`max`/`abs`,
  `cond_to_mask`, lowp `div255*` for x86, lowp compare-select `min`/`max`, bitwise lowp
  `if_then_else`, `scaled_mult`) and `S<T>`. `rp` is `pub` (A3's stage stamping lives in the same
  crate, and a public module keeps unused-yet primitives free of dead-code noise); calling a
  native primitive outside its tier's feature context still needs `unsafe` plus a token.
- **`si!`** in each native tier adds `#[target_feature(enable = Token::FEATURES)]`, `#[inline]`,
  `#[must_use]` and a generated `# Safety` doc section (clippy's `missing_safety_doc` treats safe
  `#[target_feature]` functions as unsafe to call). Scalar's and the models' `si!` add only
  `#[inline]`/`#[must_use]`. `Sse41` re-exports the `Sse2` functions it shares (calling a subset
  feature function is safe) and redefines `floor_`/`ceil_` (`roundps`), `pack_u32` (`packusdw`),
  `rcp_fast`/`rsqrt` (raw estimates), `div_u32` (`pminud` clamp), `to_half`, and lowp `floor_`,
  `min_intr_i`/`max_intr_i`, `min_intr_u16`/`max_intr_u16`, `scaled_mult` (`pmulhrsw`).
- **Names added to the §2.4 list:** `sqrt_` (per tier: the x86 models need `sqrtps`'s NaN rules),
  `cast_f` (Skia's `cast(U32) -> F`: a *signed* `cvtdq2ps` on SIMD tiers but an unsigned C cast
  on Scalar, a tier difference missing from §1.3), `min_i max_i min_u max_u abs_i` (portable),
  lowp `min_f max_f min_i max_i min_u16 max_u16` (compare-select), `min_intr_*`/`max_intr_*`,
  `if_then_else_{f,i,u16,u32}`, `trunc_`, `to_i32`, `sqrt_`.
- **`from_half`/`to_half`** are common code in Skia (`opts#L1651-L1695`), so they are stamped
  into each tier by `soft_half!` with that tier's `if_then_else`/`pack`. Consequence missing from
  §1.3/§1.5: `to_half` ends in `pack(U32)`, so for `|f| >= 2^32`, `±inf` and NaN (where
  `(s>>16) + (em>>13) - (112<<10)` exceeds 16 bits) **Sse41 saturates to `0xFFFF`** while Scalar
  and Sse2 truncate (`f32::MAX → 0x3BFF`, `+inf → 0x3C00`).
- **Scalar semantics:** comparisons on `S<T>` give 0/1; C float → int casts are Rust's `as`
  (saturating, NaN → 0), i.e. wasm's `trunc_sat` (the `x64-scalar` proxy's `cvttss2si` differs
  for out-of-range inputs, R5); `fminf`/`fmaxf` are musl's (NaN ignored, `-0 < +0`), the libc of
  the real wasm target (the MSVC CRT of the proxy may order ±0 differently). `div_i32`/`div_u32`
  port the generic `div_fn` (`x/0 → x/-1`, `INT_MIN/-1 → INT_MIN/-2`, `u/0 → u/0xFFFFFFFF`).
- **Models per estimate source.** A model module is instantiated once per `Estimates` value by
  mounting the same body (`model_*/imp.rs`, via `#[path]`) under a submodule that defines
  `const EST` (or, for `model_sse41`, its `model_sse2` base): `model_sse2::{host, amd_zen4}`,
  `model_sse41::{host, amd_zen4}`. `x86_model::rcpps`/`rsqrtps` take the `Estimates`: `Host` runs
  the host's instruction, `AmdZen4` calls A2d's `estimates::amd_zen4::{rcp, rsqrt}` (pure, any
  host, Miri). The twin tests compare `Native` with both `Model(Host)` and `Model(AmdZen4)`; the
  latter only on estimate-free primitives when the host's fingerprint is not the oracle host's.
- **NaN payloads.** The models never let a NaN come out of Rust arithmetic (unspecified in Rust,
  randomized by Miri, different on Arm). One thing is unspecified on x86 itself: when two NaNs
  meet in one *commutative* operation, LLVM may commute `fmul`/`fadd` operands, for us and for
  Skia's clang (the release codegen of `mad(dr, 1 - a, r)` emits `mulps (1-a), dr` and
  `addps p, r`). The `mad`/`nmad` twin tests therefore compare only NaN-ness when two NaN
  operands meet (`test_support::mad_nan_ambiguous`); everything else is compared bit for bit.
  The same caveat applies to portable `Vec` arithmetic in stages (A3+): stage-twin tests must
  either avoid inputs where two NaNs meet or compare NaN-ness there, and models run on Arm hosts
  inherit Arm's NaN choice in portable arithmetic.
- **Codegen** (release, checked): a `sse2` stage computing `mad(d, 1 - a, r) * rcp_fast(a)`,
  `min_f`, `bit_cast`, `if_then_else_f`, `floor_` compiles to straight `rcpps`/`mulps`/`subps`/
  `minps`/`cmpltps`/`cvttps2dq`/`andps`/`andnps`/`orps` with one load per input and one store; the
  array round trips and the byte-wise `bit_cast` vanish. `pack_u32` is `pslld`/`psrad`/
  `packssdw`/`movq`.
- **`unsafe`:** four blocks in `rp/lanes/x86.rs` (`_mm_loadu_ps`, `_mm_storeu_ps`,
  `_mm_loadu_si128`, `_mm_storeu_si128`); every other register shape goes through `bit_cast`.
  Tests add one block per (native tier, harness) call after taking the token.
- **Tests** (`rp/lanes/tests.rs`): each primitive is stamped into a test harness in every lane
  module (`lane_harness!`/`lowp_harness!`), so native and model run identical driver code. Native
  vs `Model(Host)` under `force_tier`, bit for bit: unary float ops on specials + a strided sweep
  of all 2³² patterns (every 16411th in debug, 257th in release), every 16-bit value for
  `from_half`/`pack_u16`/`div255*`, special-value cross products + random lanes for binary/ternary
  ops, random and non-canonical masks for selects/`any`/`all`. `SKIA_RUST_EXHAUSTIVE=1` runs every
  unary float primitive (highp and lowp) on all 2³² inputs, native vs both models (0
  mismatches on the Zen 4 oracle host for Sse2 and Sse41). Known-answer tests per tier cite the
  C++ line they come from and run on Scalar, the models (also under Miri) and the native tiers.
  `vx::Vec::bit_cast` (size checked at compile time) is built from the sealed `Lane` trait's new
  `ne_byte`/`from_ne_byte_fn`.

**As implemented in A2b** (`rp/lanes/{ml3,ml4}.rs`, `rp/lanes/model_{ml3,ml4}/`):

- **Modules.** `rp::lanes::{ml3, ml4}` (`x86_64` only) and the models `model_ml3::{host,
  amd_zen4}`, `model_ml4::{host, amd_zen4}` (same `imp.rs` mounting as A2a). They export exactly
  the A2a names (no additions); every native function carries the tier token's full feature
  string (`Ml3Token::FEATURES` / `Ml4Token::FEATURES`). Ml3's lowp (`N = 16`) splits into two
  highp halves wherever Skia does (`max_intr`/`min_intr` on `F` and `I32`, `rcp_precise`,
  `sqrt_`, `floor_`, `trunc_`); Ml4's lowp has the highp width (`N = LOWP_N = 16`) and calls the
  highp primitives directly. `rcp_approx`/`rsqrt_approx` are `vrcpps`/`vrsqrtps` (Ml3) and
  `vrcp14ps`/`vrsqrt14ps` (Ml4); the Ml4 models take them from `x86_model::{rcp14ps,
  rsqrt14ps}` (`Estimates::Host` on AVX-512 hosts, `Estimates::AmdZen4` =
  `estimates::amd_zen4::{rcp14, rsqrt14}` anywhere). Ml4's `floor_`/`ceil_` use
  `_mm512_roundscale_ps::<_MM_FROUND_FLOOR/CEIL>`, which is what clang's `_mm512_floor_ps`
  expands to (Rust has no `_mm512_floor_ps`).
- **Fused `mad`.** Highp `mad`/`nmad` and Ml3/Ml4's `rcp_precise` (`fnmadd(v, e, 2) * e`) are
  single FMAs; lowp `mad`/`nmad` stay `a + f*m` (unfused) on these tiers too. The models use
  `x86_model::{fmadd, fnmadd}`: `f32::mul_add` on non-NaN inputs (correctly rounded on every
  host and under Miri), invalid operations → the indefinite, and a single NaN operand comes out
  quieted **with its own sign** (measured: `vfnmadd` never negates a NaN, and a NaN operand wins
  over an invalid `0 * inf`). Two codegen-dependent cases, both also present in Skia's clang:
  which NaN an FMA returns when several operands are NaN (the compiler picks the
  `132`/`213`/`231` form; twin tests compare NaN-ness, `test_support::fma_nan_ambiguous`), and the
  sign of `nmad`'s/`rcp_precise`'s NaN when the negated operand is NaN: `_mm*_fnmadd_ps(a, b, c)`
  is `fma(-a, b, c)` in both Rust and clang; optimized builds fold the negation into `vfnmadd`
  (matching the model bit for bit, checked in release, exhaustively for `rcp_precise`), but an
  unoptimized build materializes it and flips the NaN's sign. Debug twin tests therefore accept a
  sign-only difference there (`NanRule::FusedNeg`); release builds compare every bit.
- **Selects and masks.** `if_then_else` is `vblendvps` (Ml3) / `vptestmd` + masked blend (Ml4):
  only the sign bit of the condition matters. `any`/`all` are `vptest` over the whole 256-bit
  register on Ml3 (any bit set / every bit set) and `vptestmd` per lane on Ml4 (some / every lane
  nonzero), as §1.3 says; known-answer tests pin non-canonical masks for all four x86 tiers.
- **F16C.** `x86_model::{cvtph2ps, cvtps2ph}` model `vcvtph2ps`/`vcvtps2ph` with
  `_MM_FROUND_CUR_DIRECTION` under the default MXCSR: exact half → float (denormals kept, NaN
  quieted with its payload shifted up), float → half rounded to nearest even with half
  denormals, `|f| ≥ 65520` (incl. `inf`) → `±inf`, NaN quieted keeping the top 10 payload bits.
  So none of the software path's quirks (§1.5, A2a's Sse41 `pack` saturation of `to_half`)
  exist on Ml3/Ml4.
- **`div_fn`.** `div_i32` is the `f64` path on both tiers (`x / 0`, `INT_MIN / -1` →
  `0x80000000`); Ml3's `div_u32` clamps both operands to `INT_MAX` with `pminud` (= Sse41). Ml4's
  `div_u32` converts with `vcvtudq2pd` and back with `vcvttpd2udq`, so it is the exact quotient,
  and `x / 0` (and `0 / 0`) gives **`0xFFFFFFFF`**, the unsigned integer indefinite (Skia's
  comment there says `INT_MIN`; the instruction says otherwise, measured and modelled by
  `x86_model::cvttpd2udq`).
- **`unsafe`:** eight more blocks in `rp/lanes/x86.rs` (`_mm256_{loadu,storeu}_{ps,si256}`,
  `_mm512_{loadu,storeu}_{ps,si512}`), the same one-op pattern as A2a's four; every other register
  shape is a `bit_cast` or an intrinsic on those. Tests add four blocks (native harness calls
  after `Ml3Token`/`Ml4Token`).
- **Tests.** The A2a harnesses now cover all four x86 tiers (`X86`), native vs `Model(Host)` and
  `Model(AmdZen4)`; known-answer tests take per-tier expectations (`kat5`: Scalar, Sse2, Sse41,
  Ml3, Ml4; `kat_lowp4`) for the new differences: fused vs unfused `mad`/`nmad` (also lowp
  staying unfused), sign-bit selects, `any`/`all`, F16C conversions, Ml4's exact `div_u32`.
  `x86_model` has its own tests (FMA NaN rules measured on the oracle host, an all-halves
  round trip). `SKIA_RUST_EXHAUSTIVE=1` (release, Zen 4 oracle host): 19 unary primitives
  (14 highp, 5 lowp) on all 2³² inputs against both models per tier, 1.63·10¹¹ lane comparisons
  per tier, **0 mismatches** for Sse2, Sse41, Ml3 and Ml4.
- **GM harness.** With `model_ml4` in place `tests/gm`'s policy no longer reports `Ml4` as not
  checkable on hosts without AVX-512 (or with other AVX-512 estimates): it runs
  `Model(AmdZen4)` like the other x86 tiers (§4.6 item 4; `docs/PORTING.md` §11 updated).

### 2.5 Stage code: written once, stamped per tier

Skia compiles one header N times in N namespaces. We do the same with `include!`:

```rust
// crates/skia-rust-simd/src/rp/tiers/ml3.rs
use crate::rp::lanes::ml3::*;
/// Every function in the stage sources is wrapped in `si! { … }` (Skia's `SI`).
macro_rules! si {
    ($($item:item)*) => { $(
        #[target_feature(enable = "sse2,ssse3,sse4.1,sse4.2,avx,avx2,bmi1,bmi2,f16c,fma")]
        #[inline]
        $item
    )* };
}
pub(crate) mod highp { use super::*; include!("../highp/mod.rs"); }
pub(crate) mod lowp  { use super::*; include!("../lowp/mod.rs"); }
```

```rust
// crates/skia-rust-simd/src/rp/highp/blend.rs  (included by every tier)
// Port of: src/opts/SkRasterPipeline_opts.h#L2474-L2478 (chrome/m156)
si! {
    fn colorburn_channel(s: F, d: F, sa: F, da: F) -> F {
        if_then_else_f(d.eq_mask(da), d + s * inv(da),
        if_then_else_f(s.eq_mask(F::splat(0.0)), /* s + */ d * inv(sa),
            sa * (da - min_f(da, (da - d) * sa * rcp_fast(s))) + s * inv(da) + d * inv(sa)))
    }
}
```

- The included sources name `F`, `mad`, `rcp_fast`, … unqualified; each tier module brings its own
  into scope, exactly like `SK_OPTS_NS`. Every instantiation is fully type-checked; a type error
  shows up once per tier.
- `macro_rules!` is textually scoped, so the `si!` defined in a tier file is the one the included
  text sees. No proc-macro crate is needed.
- Instantiations: native `scalar`, `sse2`, `sse41`, `ml3`, `ml4`, `neon` (cfg'd by
  `target_arch`), plus model twins (`model_sse2`, …, §2.8) under `cfg(any(test, feature =
  "models"))`. `scalar` is both a real tier and its own model.
- Stage sources are split the way Skia's file is: `highp/{mod,math,memory,blend,color,geometry,
  sampling,gradient,sksl_*}.rs`, `lowp/{mod,…}.rs`. Each file starts with its `// Port of:` ranges.

### 2.6 Program representation and execution

**Ops and contexts.** `Op` mirrors `SkRasterPipelineOp` 1:1 (same names, same order, generated
from one `rp_ops!` list transcribed from `SkRasterPipelineOpList.h`, so `kNumRasterPipelineLowpOps`
etc. stay meaningful). A program is a slice of typed stages — the op plus its context, borrowed:

```rust
pub enum Stage<'a> {
    SeedShader,
    UniformColor(&'a UniformColorCtx),
    MatrixPerspective(&'a [f32; 9]),
    Load8888(MemoryCtx),            // `MemoryCtx { slot: MemSlot, stride: i32 }`
    Gather8888(&'a GatherCtx<'a>),  // read-only pixels: `&'a [u8]`, width, height, stride…
    Srcover,
    BranchIfAnyLanesActive(&'a BranchCtx),
    Callback(&'a dyn Fn(&mut CallbackCtx, usize)),
    // … one variant per op; ops with no context are unit variants
}
```

Writable memory is not borrowed by the program (a compiled pipeline is shared and reused); it is
bound per run, which is also exactly what tail patching needs:

```rust
pub struct MemoryBindings<'m> { slots: Vec<MemView<'m>> }   // usually 1–2 entries; no new deps
pub enum MemView<'m> { Read(&'m [u8]), Write(&'m mut [u8]) }   // + stride in pixels
```

`skia_rust_core::raster_pipeline::RasterPipeline` (the builder) records `Stage`s, registers each
`MemoryCtx` with `bpp`/load/store flags like `addMemoryContext`, decides lowp vs highp with
`skia_rust_simd::rp::has_lowp(tier, op)` (§1.6), and `compile()`s into a
`CompiledPipeline { tier: Selection, lowp: bool, stages: Box<[Stage]>, scratch: Box<[Scratch]> }`
whose scratch buffers are zeroed once (matching `compile()`), while `run()` builds a fresh one
(matching `run()`).

**Interpreter.** Stable Rust has no guaranteed tail calls (`become` is unstable), so the C++
`musttail` chain is replaced by a `match` loop inside one feature-enabled function per tier and
precision:

```rust
// rp/highp/mod.rs — included per tier
si! {
    pub(crate) fn run(prog: &[Stage<'_>], x0: usize, y0: usize, xl: usize, yl: usize,
                      mem: &mut Mem<'_, '_>) {
        for dy in y0..yl {
            let mut dx = x0;
            while dx + N <= xl { chunk(prog, dx, dy, 0xFF, mem); dx += N; }
            if dx < xl {
                let tail = xl - dx;
                mem.patch(dx, dy, tail);          // opts#L1697-L1715
                chunk(prog, dx, dy, tail as u8, mem);
                mem.restore(dx, dy, tail);        // opts#L1717-L1735
            }
        }
    }

    fn chunk(prog: &[Stage<'_>], dx: usize, dy: usize, tail: u8, mem: &mut Mem<'_, '_>) {
        let mut r = Regs::default();              // r g b a dr dg db da: F; base
        let mut pc = 0;
        loop {
            // generated from rp_ops!: one arm per highp op
            match &prog[pc] {
                Stage::JustReturn => return,
                Stage::Srcover => { srcover(&mut r); pc += 1; }
                Stage::Load8888(c) => { load_8888(&mut r, mem.view(c, dx, dy)); pc += 1; }
                Stage::BranchIfAnyLanesActive(c) => { pc = pc.wrapping_add_signed(branch_if_any(&r, c)); }
                // StackCheckpoint / StackRewind: no-ops (they only bound C++ stack depth)
                …
            }
        }
    }
}
```

- The eight registers are locals of one function, so LLVM keeps them in vector registers across
  stages (Skia gets the same from its register calling convention). Small stages inline into the
  `match`; LLVM may keep large ones out of line (they then take `&mut Regs`).
- `stack_checkpoint`/`stack_rewind` (`opts#L1889-L1992`) exist only to reclaim C++ stack in long
  SkSL programs; in a loop they are no-ops with identical results. `Stage::JustReturn` ends the
  chunk.
- lowp keeps Skia's register aliasing exactly: `x`/`y` floats are `join`ed from/`split` into the
  `r,g`/`b,a` `U16` registers by GG/GP stages (`opts#L5579-L5685`), via byte-level bit casts.
- Highp pixel access is safe slicing: `mem.view(c, dx, dy)` returns the row slice at
  `(dy*stride + dx)*bpp`, or the scratch buffer during the tail (the patched pointer in Skia);
  stages load `N` pixels with `<[u8; 4*N]>::try_from(&row[..4*N])`. One bounds check per memory
  stage per chunk.
- **Performance plan.** A micro-benchmark against Skia's `SkRasterPipelineBench` lands with A3. If
  the interpreter loses on short pipelines, add *fused programs*: for the hottest op sequences
  (e.g. lowp `seed_shader, uniform_color, load_8888_dst, srcover, store_8888`) a generated function
  that calls the same stage functions in a fixed order. Same stage code, so results are identical by
  construction; a test runs every fused program against the interpreter.

### 2.7 The builder (`SkRasterPipeline.cpp`)

Ported 1:1 in core: `append`, `appendMatrix` (op choice by matrix type), `appendConstantColor`
(`uniform_color` vs `unbounded_uniform_color`, black/white shortcuts), `appendSetRGB`,
`appendLoad`/`appendLoadDst`/`appendStore` (color-type switch incl. swizzles and `alpha_to_red`),
`appendClampIfNormalized`, `appendTransferFunction` (skcms classification), `appendStackRewind`,
`buildLowpPipeline`/`buildHighpPipeline`, `run`, `compile`, `dump` (Skia's op names, used to diff
against the oracle's stage dumps, §4.3). `gForceHighPrecisionRasterPipeline` becomes a field of the
builder set by tests, not a global.

### 2.8 Scalar twins = model tiers

For each SIMD tier there is a model lane module `lanes/model_<tier>.rs` with the same `N` and the
same names, implemented per lane in scalar Rust with that tier's exact semantics, and **no target
features**:

| Primitive | Model implementation |
|---|---|
| `min_f`/`max_f` x86 | `if a < b { a } else { b }` / `if a > b { a } else { b }` |
| `min_f`/`max_f` Neon | NaN-propagating, `-0 < +0` (`FMIN`/`FMAX` pseudocode) |
| `mad` (fused tiers) | `f32::mul_add` (correctly rounded on every host, software or hardware) |
| `iround` x86 | NaN or out of `i32` range → `i32::MIN`, else `round_ties_even() as i32` |
| `trunc_`/casts x86 | NaN or out of range → `i32::MIN`, else truncation |
| `floor_` Sse2 | `cvtt` model + compare, as in `opts#L1069-L1070` |
| `rcp_approx`/`rsqrt_approx` x86 | `Estimates::AmdZen4`: `estimates::amd_zen4::{rcp, rsqrt}` (4096/8192-entry tables + specials) and `::{rcp14, rsqrt14}` (Ml4; Intel's reference algorithm); `Estimates::Host`: `_mm_rcp_ss`/`_mm_rcp14_ss` on one lane (native hosts only) |
| `rcp_approx`/`rsqrt_approx` Neon | port of Arm ARM `RecipEstimate`/`RecipSqrtEstimate`; `FRECPS`/`FRSQRTS` fused |
| `to_half` F16C/Neon | IEEE f32→f16 RNE with denormals and NaN quieting (as `vcvtps2ph`/`FCVTN`) |
| `if_then_else` Ml3/Ml4 | select on the sign bit only |
| `any`/`all` | per tier, as in §1.3 (sign bits on Sse*, whole-register bits on Ml3, per-lane nonzero on Ml4/Neon) |

Stamping the same stage sources with a model lane module gives a complete twin of every stage
for free, with only the ~55 primitives written twice. Uses:

1. **Scalar-twin test** (CLAUDE.md rule 5): for every stage and tier, random registers and
   contexts through `Native` and `Model` must be bit-identical; primitives are additionally
   checked exhaustively over all 2³² `f32` inputs on the server (seconds per primitive).
2. **Any tier on any host**: CI runners without AVX-512 check `Ml4` goldens with
   `Backend::Model(AmdZen4)`, arm64 runners check x86 tiers, x86 runners check `Neon`.
3. **Miri** runs the model instantiations (no intrinsics apart from `Estimates::Host`).

**Estimate models (A2d).** The estimate tables are data generated by
`cargo xtask cpu-probe --dump-tables` on the oracle host and committed under
`crates/skia-rust-simd/src/estimates/amd_zen4/` (`rcpps.bin` 4096 × u32 LE, `rsqrtps.bin`
2 × 4096, `estimates.txt` with the host and its fingerprints), decoded at compile time with
`include_bytes!` into `estimates::tables::AMD_ZEN4`, whose `rcp_approx`/`rsqrt_approx` add the
special-value and exponent rules of §1.4. `estimates::recip14` models `vrcp14ps`/`vrsqrt14ps`
(§6 R2). The API model lanes call is `estimates::amd_zen4::{rcp, rsqrt, rcp14, rsqrt14}(f32) ->
f32`. Verification:

- `exhaustive_amd_zen4_vs_host` (`#[ignore]`d; on the oracle host:
  `cargo test -p skia-rust-simd --release -- --ignored exhaustive_amd_zen4`, ~10 s on 16
  threads): all 2³² inputs, model vs host instruction. Result on the oracle host: **0
  mismatches** for each of `rcpps`, `rsqrtps`, `rcp14`, `rsqrt14`. It fails (never passes
  vacuously) on a host whose fingerprints differ.
- Always run: the models reproduce the five §1.4 fingerprints (host-independent, 5 × 2²³
  inputs), known oracle-host values incl. specials (also under Miri), and a ~10⁶-input sample
  vs the host when the host's fingerprints match.

---

## 3. Keeping `unsafe` minimal

Facts (rustc 1.99, checked with probes):

- Value-only intrinsics (`_mm256_fmadd_ps`, `_mm_rcp_ps`, `__cpuid`, …) are **safe to call inside a
  `#[target_feature]` function** that enables their features; closures inside such a function
  inherit the features.
- Calling a `#[target_feature]` function (ours or an intrinsic) from a function without those
  features needs `unsafe`, **even for baseline features** like `sse2`.
- A `#[target_feature]` function coerces only to an `unsafe fn` pointer.
- Intrinsics taking raw pointers (`_mm*_loadu_*`, `_mm*_storeu_*`, gathers, `vld1q_*`) are always
  `unsafe`.
- There is no safe `From<[f32; 8]> for __m256` (or any array ↔ register conversion).

So `unsafe` is needed in exactly these places:

### 3.1 Tier entry points (~14 blocks)

One call per (tier, entry) after detection: `{sse2, sse41, ml3, ml4, neon} × {highp::run,
lowp::run}`, plus `blit_row`/`blit_mask`/`memset` entries. Safety argument: the caller holds a
token (`Sse41Token`, `Ml3Token`, `Ml4Token`) that is only constructible after
`is_x86_feature_detected!` confirmed every feature in the entry's `#[target_feature]` list, or the
feature is in the compile-time baseline (`cfg(target_feature = "sse2")`, `"neon"`), asserted with
`const _: () = assert!(cfg!(target_feature = "sse2"));`.

### 3.2 Array ↔ register conversions (~10 per SIMD tier, ~50 total)

```rust
si! {
    pub(crate) fn to_reg(v: F) -> __m256 {
        // SAFETY: `v.0` is a live `[f32; 8]`, so its pointer is valid for 32 bytes of reads;
        // `_mm256_loadu_ps` has no alignment requirement.
        unsafe { _mm256_loadu_ps(v.0.as_ptr()) }
    }
    pub(crate) fn from_reg(r: __m256) -> F {
        let mut out = F::splat(0.0);
        // SAFETY: `out.0` is a live `[f32; 8]`, valid for 32 bytes of writes; no alignment needed.
        unsafe { _mm256_storeu_ps(out.0.as_mut_ptr(), r) };
        out
    }
}
```

One pair per register shape (`__m128`/`__m128i` halves, `__m256`/`__m256i`, `__m512`/`__m512i`,
`float32x4_t`, `uint16x8_t`, …). LLVM removes the round trip (verified in the codegen check above).

### 3.3 Everything else is safe

- **Pixel memory**: bounds-checked slices and `try_from` into arrays; no pointer arithmetic.
- **Gathers**: portable lane loops with checked indexing. Results are identical to `vgatherdps`
  (pure loads); hardware gathers can be added later only with a benchmark and an explicit
  `assert!` on the maximum index before the single `unsafe` call.
- **Integer division** (`div_fn`): the f64 trick operates on registers via §3.2.
- **Detection**: `is_x86_feature_detected!`, `std::arch::is_aarch64_feature_detected!`.
- **Public API**: `skia_rust_simd` exposes `Tier`, `Selection`, `Stage`, contexts, `Program`,
  `MemoryBindings`, `run_*`, kernels on slices. No raw pointers cross the crate boundary.

All blocks follow docs/UNSAFE.md (one op, `// SAFETY:`); `clippy::undocumented_unsafe_blocks` and
`multiple_unsafe_ops_per_block` already deny violations. Expected total: ~70 blocks, two kinds,
reviewable as two patterns.

---

## 4. Exactness testing

### 4.1 Layers

| Layer | What | Where it runs |
|---|---|---|
| Primitive exhaustive | each primitive, all 2³² inputs (unary) / structured sweeps (binary), Native vs Model | server nightly; sampled in PR CI |
| Stage twin | every stage × tier, random + special-value registers and contexts, Native vs Model | PR CI on tiers the runner has |
| Stage oracle (`rp-diff`) | every stage × tier vs Skia's actual stage (§4.2) | server; results published, compared in CI |
| Ported unit tests | `SkRasterPipelineTest` (41), `SkRasterPipelineOptsTest` (11), `F16StagesTest`, `ParametricStageTest` (14), … run once per tier | CI, per tier the runner supports (Model for the rest) |
| Stage-list diff | our `RasterPipeline::dump()` vs the oracle's per draw (§4.3) | on mismatch |
| GM | SHA-256 per (GM, config, tier) vs goldens | CI per tier; server all tiers |

Special values for every float input: ±0, ±denormal min/max, ±FLT_MIN, ±1, 0.5±ulp, 2.5, 255.5,
65535.5, 2³¹, −2³¹, 2³², ±FLT_MAX, ±inf, quiet/signalling NaN with both signs and payloads. Lane
patterns vary per lane so lane-order bugs (`load4`, `pack`, `split`/`join`) show up.

### 4.2 `oracle/rp-diff`: a per-stage harness (yes, we need one)

Modelled on `oracle/skcms-diff`. GMs exercise stages only in combination and rarely hit edge cases;
the stage-level harness is where tier semantics get pinned down.

- **C++ side.** One translation unit per Skia path: `#define SK_OPTS_NS rpdiff_<t>` (and, for
  scalar, `#define SKRP_CPU_SCALAR`) then `#include "src/opts/SkRasterPipeline_opts.h"`, compiled
  with the same clang-cl and flags as the oracle (`/clang:-ffp-contract=off`; `-msse4.1`;
  `/arch:AVX2` ≈ ml3; `/arch:AVX512` ≈ ml4). Each TU exports
  `extern "C" void rpdiff_<t>_run(const Case*)` that builds an `SkRasterPipelineStage[]` from the
  op table exactly like `SkOpts.cpp` (`M(st) (StageFn)SK_OPTS_NS::st`) and calls
  `SK_OPTS_NS::start_pipeline` / `lowp::start_pipeline`. Link against the oracle's `x64-sse2` static
  libraries for the few out-of-line symbols.
- **Cases.** `load_src`/`load_dst` (both precisions have them, `SkRasterPipelineOpList.h`) feed
  arbitrary registers, the op under test runs with a serialized context, `store_src`/`store_dst`
  read the result. Widths `w ∈ {1, N-1, N, N+1, 3N+2}` exercise the tail path; heights ≥ 2 exercise
  scratch persistence.
- **Outputs.** Per case: the output bytes' hash, plus full bytes for the first mismatch (like
  `FIRST=1` in skcms-diff). Results are generated on the server per tier and published with the
  goldens (`rp-<tier>.json`), each tagged with the host's estimate fingerprint.
- **Rust side.** `oracle/rp-diff/rust` replays the same case file through `skia_rust_simd` under
  `force_tier` and compares hashes, so CI checks stage exactness without a C++ toolchain.
- Neon and wasm cases are produced the same way on their oracle hosts once those exist.

### 4.3 DM stage-list dump

Add to `oracle/patches/skia-oracle.patch`: when `SKIA_ORACLE_RP_DUMP=<file>` is set, every
`SkRasterPipeline::compile()`/`run()` appends `<result id> <highp|lowp> <op> <op> …` (names from
`SkRasterPipeline::GetOpName`). `cargo xtask oracle rp-dump <tier> <result-id>` extracts it. Our
`RasterPipeline::dump()` prints the same format, so the first divergent draw/stage of a failing GM
is found by `diff`, before looking at any arithmetic. This is the "raster pipeline stages" dump
CLAUDE.md refers to.

### 4.4 GMs per tier

`tests/gm` (new crate, task A7) ports DM's `GMSrc::draw` + `RasterSink` for configs `8888`, `565`,
`f16` (color type, alpha type and color space taken from the goldens' `meta.json`). For each GM and
config it renders once per `Selection` and compares the SHA-256 with that tier's golden
(`Tier::oracle_tiers()[0]`). On this host (Zen 4) all four x86 tiers run natively; `Scalar` and
`Neon` run as models. A full pass is 2,727 renders × 6 tiers.

**As implemented in A7** (usage: `docs/PORTING.md` §11):

- **Crate.** `tests/gm` is the crate `skia-rust-gm` (`publish = false`): `canvas` (the seam),
  `sink` (`GMSrc` + `RasterSink`), `goldens`, `check` (per-tier loop, verdicts), `diff`, `gm`
  (the ports, `gm::<file_snake>`), and the binary `gm-verify`. It is excluded from the wasm CI
  build (host tooling: files, `curl`, `tar`, `zstd-sys`).
- **Registration.** `def_gm!` and the `def_simple_gm*!` family mirror `DEF_GM`/`DEF_SIMPLE_GM*`;
  each registers a `GmRegistration { module_path, name, factory }` with the `inventory` crate
  (Skia's static `sk_tools::Registry`) and defines a `#[test]` of the same name. The `GM` trait
  holds the overridable virtuals (`name`, `size`, `bg_color`, `on_once_before_draw`, `on_draw`,
  `on_draw_with_error`, `modify_surface_props`, `on_gpu_setup`); `GmInstance` is the
  non-virtual driver (`draw` = `drawBackground` (`drawColor(bg, kSrc)`) + `drawContent`
  (`onDraw` inside `SkAutoCanvasRestore`), `onceBeforeDraw`, `gpuSetup` with no GPU context).
- **Surface stub until D6.** `canvas::{Canvas, Surface, SurfaceProps, PixelGeometry, BlendMode}`
  have `skia-safe`'s shape; the stub `Canvas` does `save`/`restore`/`restore_to_count` and
  `draw_color`/`clear` that replace every pixel (`Pixmap::erase_4f`), and panics on anything
  else. D6 replaces the module with re-exports of the real types; GM ports don't change.
- **DM semantics.** `RasterSink::draw`: skip empty sizes (`"Skipping empty source: <name>"`),
  zeroed pixels of `SkImageInfo::Make(size, colorInfo())` with the config's color type
  (`8888` = `kN32` (BGRA), `565`, `f16`), premul corrected by `SkColorTypeValidateAlphaType`
  (565 → opaque), null color space (no config has a color-space via), surface props
  `(0, kRGB_H)` adjusted by the GM, a fresh GM per query as `GMSrc` does. `DrawResult::Skip`
  writes nothing (a match only if the oracle has no result either); `Fail` is always a failure
  (DM writes a failure-message image with text). Bytes are extracted like `OracleDump`
  (`minRowBytes` per row, tightly packed).
- **Goldens.** Local `goldens/<commit>/` (workspace, main checkout of a worktree, or
  `$SKIA_RUST_GOLDENS`), else the release `hashes-<mNNN>.json` verified against
  `inventory/goldens.lock` and cached in `target/goldens/`; objects (for diff PNGs, `png` crate)
  come from the local store or the release tar, fetched only on a mismatch.
- **Tier policy (§4.6).** Each `Tier` renders once per config under `force_tier` and is compared
  with *every* oracle tier in `oracle_tiers()` that has goldens (not only the first, so a class
  split shows up as a GM failure too). x86 tiers run `Native` when the host's fingerprints match
  `AMD_ZEN4` for the tier, else `Model(AmdZen4)` (`Ml4` too since A2b's `model_ml4`; before it,
  `Ml4` without a match was not checkable). `Neon` falls back to `Model(Arm)`; it has no goldens yet.
  `cpu-x64-scalar` is a proxy (§4.5): compared and reported, never decisive.
- **N32 byte order.** `8888` is `kN32` (BGRA on Windows, RGBA elsewhere) and the default goldens
  are `BGRA_8888` (Windows oracle host). Bytes are never swizzled. Hosts whose N32 is RGBA
  compare `8888` with the RGBA oracle variants instead: builds with `-DSK_R32_SHIFT=0`, tiers
  suffixed `-rgba`, listed by `Tier::oracle_tiers_rgba()` (`sink::uses_rgba_goldens(config,
  host_n32)`, pure; `Options::host_n32` injects the order; `TierPlan::oracle_tiers_for`). Only the
  class representatives are built (`x64-sse2-rgba` at baseline/ml3/ml4, `x64-sse41-rgba`,
  `x64-scalar-rgba`, the last a proxy like `cpu-x64-scalar`), as classes of their own in
  `check-classes`. A tier with no golden of the host's byte order (`Neon`, or goldens published
  without the variants) is *not checkable* for `8888`, reported but neither a pass nor a
  failure. `565`/`f16` are byte-order independent and always use the default tiers.
- **Verdicts and the manifest.** `passing` = every config matches on every non-proxy oracle tier
  with goldens and all were checkable; any mismatch, draw failure, panic, unexpected skip or
  missing golden is `failing`; otherwise `not-checkable`. `cargo xtask inventory verify` runs
  `gm-verify`, maps registry keys `gm::<file_snake>::<name>` to manifest ids
  `gm/<file>.cpp::<name>`, marks `passing`/`failing` with `--update`, and treats
  `not-checkable` as neither a pass nor a regression.
- **First port.** `gm/fiddle.cpp::fiddle` (draws nothing; solid white goldens) passes on all
  19 x64 oracle tiers and the proxy, on this host natively. A harness self-test also checks a
  background-only stand-in for `path_effect_empty_result` against the real goldens.

`cargo xtask oracle check-classes` (run on every golden publish) asserts that the oracle tiers
mapped to one `Tier` still have identical hash files; a new difference means a fifth behaviour and
blocks the pin bump until this design is updated.

### 4.5 Scalar and Neon goldens before their oracles exist

- **Scalar proxy:** an extra oracle build `x64-scalar` with `extra_cflags =
  ["/clang:-DSKRP_CPU_SCALAR"]` (opts honours a predefined `SKRP_CPU_*`, `opts#L76-L79`), run at
  the baseline level. It reproduces Skia's scalar RP on x64; `SkOpts` kernels outside RP stay SSE2,
  which matches the portable code for premultiplied input (§1.8). libm differs from Emscripten's
  musl, so the real wasm oracle still has the final word.
- **Neon:** build DM on a hosted `ubuntu-24.04-arm` / `macos-14` runner (CPU-only, clang,
  `-ffp-contract=off`) and publish `arm64-neon` goldens; FRECPE/FRSQRTE are architectural, so the
  host model does not matter.

### 4.6 Vendor dependence and golden portability on hosted runners

Affected outputs: everything that reaches `rcp_fast`, `rcp_precise` or `rsqrt` (§1.4) — at m156,
the perspective and blend-mode GMs, on **every** x86 tier (Sse2 included, through `rcp_precise`).

GitHub's x64 runners are a mix of AMD EPYC and Intel Xeon parts; some have AVX-512, some do not.
Policy:

1. `cargo xtask cpu-probe` prints the estimate fingerprints (§1.4). Every oracle run records them in
   `toolchain.txt`; every CI job prints them.
2. If the runner's fingerprint for a tier equals the goldens' (`AmdZen4` today), that tier runs
   `Native` and must match all goldens.
3. Otherwise the tier runs `Native` with `Estimates` emulated — the native tier modules are also
   instantiated with table-driven `rcp_approx`/`rsqrt_approx` (`cfg(feature = "emulated-estimates")`,
   test-only) — so the GMs still check everything except the vendor's own tables. The model twin
   tests still compare `Native` against `Model(Host)` on that runner.
4. Ml4 on a non-AVX-512 runner uses `Model(AmdZen4)`, whose `rcp14`/`rsqrt14` model is exact on
   all 2³² inputs (§2.8, §6 R2), so those runners check every Ml4 GM. An Intel AVX-512 runner's
   `rcp14`/`rsqrt14` fingerprints are expected to equal `AMD_ZEN4`'s (same reference algorithm);
   if one does not, its Ml4 tier falls under item 3.
5. Intel SDE (PLAN §7) executes `vrcp14ps` with Intel's semantics (expected to equal Zen 4's) and
   runs `rcpps` natively on the host, so it cannot reproduce AMD-generated goldens for GMs that
   use `rcpps`/`rsqrtps`; prefer `Model` runs.
6. Optional later: generate Intel goldens for the affected tiers on an Intel runner (DM binary
   built on the server, run with `SKIA_ORACLE_CPU_CAP`) and select goldens by fingerprint.

---

## 5. Work breakdown

Sizes: S < 500 lines, M 500–1,500, L > 1,500 (Rust, excluding tests). "Unlocks" lists manifest
entries (tests ids are `tests/<File>.cpp::<Name>`) and GM groups. Prerequisites outside Phase 2 are
marked **P1** (Phase 1, still in progress: `Path`/`PathBuilder`/`SkPathRaw`, `ImageInfo`/
`ColorType`, `Pixmap`/`Bitmap`, `SkMask`).

Several raster tests are filed under `module = "core"` or `"effects"` in the manifest
(e.g. `CoreBlittersTest`, `BlitMaskClip`, half of `SkRasterPipelineTest`); the first task touching
them should set `module = "raster"` (a hand-editable field).

### Wave A — foundations (Opus; A1 first, then A2–A7 in parallel)

| ID | Task | Skia sources | Depends | Size | Unlocks |
|---|---|---|---|---|---|
| A1 | `Tier`, tokens, `detect`, `Selection`/`Backend`, `testing::force_tier`, `cargo xtask cpu-probe` (fingerprints, table dump) | `SkCpu.*`, `SkOpts.cpp`, `SkFeatures.h` | — | S | — |
| A2a | `#[inline(always)]` on `vx::Vec` ops, `bit_cast`, Scalar `S<T>`; `lanes::{scalar,sse2,sse41}` + models; exhaustive primitive tests | opts `#L102-L204`, `#L933-L1156`, `#L1539-L1749` | A1 | M | — |
| A2b | `lanes::{ml3,ml4}` + models (incl. f16c, `div_fn` f64 paths) | opts `#L334-L931`, `#L1651-L1695` | A2a | M | — |
| A2c | `lanes::neon` + model; Arm `RecipEstimate`/`RecipSqrtEstimate` port | opts `#L205-L332`, Arm ARM | A2a | M | — |
| A2d | Estimate tables (`AmdZen4` rcpps/rsqrtps incl. specials) + exhaustive verification; `rcp14` investigation (R2) | — | A1 | S | — |
| A3 | `Op` list, contexts, `Stage<'a>`, `MemoryBindings`, stamping macros, highp/lowp interpreters with tail patching, `just_return`/branch/`jump`/stack ops, `load_src`/`store_src`/`load_dst`/`store_dst`, `seed_shader`; micro-bench vs Skia | `SkRasterPipelineOpList.h`, `SkRasterPipelineOpContexts.h`, opts `#L1697-L1992`, `#L5464-L5685` | A2a | L | `SkRasterPipeline_empty`, `SkRasterPipeline_nonsense`; with B6a: `SkRasterPipeline_Jump`, `SkRasterPipeline_stack_rewind` |
| A4 | `RasterPipeline` builder in core | `SkRasterPipeline.{h,cpp}` | A3, **P1** (ColorType, Matrix) | M | `SkRasterPipeline_tail` (with B2), `SkRasterPipeline_lowp` (with B1) |
| A5 | `oracle/rp-diff` (C++ TUs, case format, driver, Rust replayer, publish) | — | A3 (Rust side) | M | per-stage oracle for all of Wave B |
| A6 | DM stage-list dump patch + `xtask oracle rp-dump`; `xtask oracle check-classes`; `x64-scalar` oracle build | `oracle/` | — | S | debugging for every GM |
| A7 | `tests/gm` harness: `def_gm!`, DM `GMSrc`/`RasterSink` semantics, per-tier loop, hash compare, diff images | `dm/DMSrcSink.cpp`, `gm/gm.{h,cpp}` | A1 (Surface stub until D6) | M | infrastructure for all GMs |

### Wave B — stages and kernels (Sonnet; all parallel after A2a + A3; each adds rp-diff cases)

| ID | Task | Skia sources (opts) | Size | Unlocks |
|---|---|---|---|---|
| B1 | 8-bit memory stages highp+lowp: `load/store/gather` for a8, 565, 4444, 8888, rg88, r8 (+`_dst`), `srcover_rgba_8888`, `swap_rb`, `alpha_to_*` | `#L2613-L2639`, `#L3091-L3218`, `#L6450-L6800`, `#L7229-L7241` | M | `SkRasterPipeline` (basic), `SkRasterPipeline_lowp`, `SkRasterPipeline_u16`, `SkRasterPipeline_swizzle` |
| B2 | Wide memory stages (highp): f16 family, f32, 16161616/a16/r16/rg1616, 1010102/xr, 10x6, 10101010_xr, `load_src_rg`/`store_src_rg` | `#L2397-L2406`, `#L3219-L3595` | M | `F16StagesTest::F16Stages`, `SkRasterPipeline_tail` |
| B3 | Blend modes highp+lowp (Porter-Duff, separable, `hue`/`saturation`/`color`/`luminosity`, colorburn/dodge/softlight/overlay/hardlight) + coverage stages (`scale_*`, `lerp_*` incl. `lerp_565` LCD, `scale_native`, `lerp_native`, `*_1_float`) | `#L2428-L2611`, `#L2897-L2970`, `#L6226-L6340`, `#L6856-L6896` | M | `BlendTest::Blend_byte_multiply`; GMs with D3: `aarectmodes`, `xfermodes*`, `hairmodes`, `srcmode`, `plus` |
| B4 | Color stages: premul/unpremul(_polar), clamps, `uniform_color`/`set_rgb`/`unbounded_*`, black/white/force_opaque, luminance, `dither`, `byte_tables`, `matrix_3x3/3x4/4x5/4x3`, `swizzle`, `emboss`, hsl/css conversions, transfer functions (`parametric`, `gamma_`, `PQish`, `HLGish`, `HLGinvish`, `ootf`) | `#L2316-L2396`, `#L2640-L2896`, `#L2971-L3090`, `#L3647-L3671`, `#L3686-L3720`, `#L5445` (+ lowp counterparts) | M | `ParametricStageTest` (14), `SkRasterPipeline_lowp_clamp01` |
| B5 | Geometry & tiling: `matrix_translate/scale_translate/2x3/perspective` (highp+lowp), `repeat`/`mirror`/`clamp`/`decal` (+`_1`, `check_decal_mask`, `clamp_x_and_y`) | `#L3565-L3646`, `#L3672-L3728`, `#L6117-L6139`, `#L6913-L6960` | S | image/sprite draws in D-wave GMs (`perspimages` needs Phase 3 shaders) |
| B6a | SkSL ops: lane masks, condition/loop/return masks, branches, `case_op`, copies (slot/uniform/constant/immutable/indirect), swizzles, `shuffle`, `exchange_src`, `store_device_xy01` | `#L4204-L4362`, `#L4440-L4720` | M | ~20 `SkRasterPipelineTest` entries (`BranchIf*`, `CaseOp`, `Copy*`, `LoadStore*Mask`, `Merge*`, `MaskOff*`, `ReenableLoopMask`, `InitLaneMasks`, `Swizzle*`, `Shuffle`, `ExchangeSrc`) |
| B6b | SkSL arithmetic: n-way/imm add/sub/mul/div/min/max/mod/mix/cmp (float/int/uint), casts, bitwise, `abs`/`floor`/`ceil`, `dot`, `matrix_multiply_*`, `smoothstep`, `refract` | `#L4720-L5330` | M | `*ArithmeticWith*Slots`, `Compare*`, `Unary*Ops`, `MixTest`, `MixIntTest`, `MatrixMultiply*` |
| B6c | SkSL transcendental: `sin/cos/tan/asin/acos/atan/atan2`, `pow/exp/exp2/log/log2`, `sqrt`, `invsqrt`, `inverse_mat2/3/4` | `#L2099-L2240`, `#L4760-L4870` | S | `SkRasterPipelineOptsTest` (11) |
| B6d | SkSL trace ops, `callback` | `#L4188-L4203`, `#L4363-L4440` | S | `SkRasterPipeline_Trace*` (4) |
| B7 | `SkOpts` kernels: `blit_row_s32a_opaque`, `blit_row_color32`, `blit_mask_d32_a8`, `memset16/32/64`, `rect_memset*` (x86 + Neon + portable, twins) | `SkBlitRow_opts.h`, `SkBlitMask_opts.h`, `SkMemset_opts.h` | M | `MemsetTest::Memset`, `SrcOverTest::SrcOver` |

Sampling/gradient/perlin stages (`bilerp_*`, `bicubic_*`, `gather_*` for shaders, `gradient`,
`evenly_spaced_*`, `xy_to_*`, `mask_2pt_conical_*`, `perlin_noise`) are Phase 3 tasks, written in
the same framework.

### Wave C — geometry and scan conversion (Sonnet; parallel; need **P1** Path/PathRaw)

| ID | Task | Skia sources | Depends | Size | Unlocks |
|---|---|---|---|---|---|
| C1 | Edges & clipping | `SkEdge.*`, `SkEdgeBuilder.*`, `SkEdgeClipper.*`, `SkLineClipper.cpp` | P1 | M | `EdgeTest` (8), `ClipperTest` (2), `ClipCubicTest` (3) |
| C2 | Non-AA fill | `SkScan.cpp`, `SkScan_Path.cpp` | C1, D1 | M | `FillPathTest::FillPathInverse`; GMs `filltypes`, `convexpaths`, `concavepaths` (with D6) |
| C3 | Analytic AA fill (m156 has no supersampler: `SkScan_AntiPath.cpp#L59-L137` dispatches to `AAAFillPath`) | `SkAnalyticEdge.*`, `SkScan_AAAPath.cpp`, `SkScan_AntiPath.cpp` | C1, D1 | L | `PathCoverageTest::PathCoverage`; GMs `aaa`, `pathfill`, `circulararcs`, `smallpaths` |
| C4 | Hairlines | `SkScan_Hairline.cpp`, `SkScan_Antihair.cpp` | C1, D1 | L | `CappedHairlinesTest` (6, with D6); GMs `hairlines`, `closedcappedhairlines`, `strokedlines` |
| C5 | Clips | `SkRasterClip.*`, `SkAAClip.*`, `SkRasterClipStack.h` | C3, D1, Region (done) | L | `AAClipTest` (11); GMs `complexclip*`, `simpleaaclip`, `aaclip` |
| C6 | Stroker | `SkStroke.cpp`, `SkStrokerPriv.cpp`, `SkStrokeRec.cpp` | P1 | L | `StrokeTest::Stroke`, `StrokerTest` (10); GMs `strokes`, `strokerects`, `overstroke`, `trickycubicstrokes` |
| C7 | Dashing | `src/utils/SkDashPath.cpp`, `src/effects/SkDashPathEffect.cpp`, `SkPathEffect` base | C6 | M | `DashPathEffectTest` (5), `AsADashTest` (3); GMs `dashing`, `dashcircle`, `dashcubics` |

### Wave D — blitters and the drawing stack

| ID | Task | Skia sources | Depends | Size | Unlocks |
|---|---|---|---|---|---|
| D1 | Blitter base: `SkBlitter` defaults (`blitAntiH2/V2`, `blitMask`, `blitRect`), `SkNullBlitter`, `SkRectClipBlitter`, `SkRgnClipBlitter`, `SkBlitterClipper`, `SkMask` | `SkBlitter.{h,cpp}` (minus `Choose`), `SkMask.*` | P1 | M | prerequisite for C2–C5 |
| D2 | Blend mode helpers + paint helpers + basic shader interface: `SkBlendModePriv` (`CheckFastPath`, `AppendStages`, coverage prescale), `SkBlendModeBlender`, `SkPaintPriv`, `ShaderBase::append_stages`, `SkColorShader`/`SkColor4Shader`/empty shader | `SkBlendMode*.cpp`, `SkPaintPriv.cpp`, `src/shaders/SkColorShader.cpp`, `SkShaderBase.cpp` | A4 | M | `PaintTest` (6, with D6) |
| D3 | `SkRasterPipelineBlitter` (all `blit*`, memset fast paths, clip-shader hook) | `SkRasterPipelineBlitter.cpp` | A4, B1–B4, B7, D1, D2 | M | the main path for every Phase 2 GM |
| D4 | Legacy blitters + `SkBlitter::Choose`/`ChooseSprite`/`UseLegacyBlitter`: `SkBlitter_ARGB32`, `SkBlitRow_D32`, `SkBlitter_A8`, `SkBlitter_Sprite` | those files, `SkBlitter.cpp#L654-L824` | B7, D3 | L | `CoreBlittersTest` (3), `BlitMaskClip::BlitAndClip` |
| D5 | `SkDraw` (+ `_vertices`, `_atlas`), `SkDevice`, `SkBitmapDevice` | those files | C2–C6, D3, D4 | L | `DrawPathTest::DrawPath`, `VerticesTest` (3) |
| D6 | `Canvas` (state, matrix, clip, draw dispatch, raster `saveLayer`), `SkClipStack`, raster `Surface` | `SkCanvas.cpp`, `SkClipStack.cpp`, `src/image/SkSurface_Raster.cpp`, `SkSurface_Base.cpp` | D5, C5 | L | `CanvasTest` (14), `SurfaceTest` (11), `ClipStackTest`, `QuickRejectTest` (2), `CappedHairlinesTest` (6) |
| D7 | Pictures: `SkRecord`, `SkRecordCanvas`/`SkRecorder`, `SkRecordDraw`, `SkRecordOpts`, `SkPictureRecorder`, `SkBigPicture`, R-tree | `src/core/SkRecord*.cpp`, `SkPicture*.cpp`, `SkRTree.cpp` | D6 | L | `RecordTest` (3), `RecordDrawTest` (9), `RecordOptsTest` (7), `RecordPatternTest` (6), `RecorderTest` (4), `PictureBBHTest` (2), part of `PictureTest` (19) |
| D8 | Pixel ops on the pipeline: `SkConvertPixels`, `Pixmap::readPixels/erase`, `Bitmap` copies | `SkConvertPixels.cpp`, `SkPixmap.cpp`, `SkBitmap.cpp` | A4, B1, B2, B4 | M | `ConvertPixelsTest`, `ReadPixelsTest` (3), `WritePixelsTest` (3), `BitmapCopyTest` (2), `PremulAlphaRoundTripTest` (3), `BitmapTest` (10) |

### Wave E — GM sweep and benches (Sonnet, wide fan-out)

After D6, agents take GM files in feature groups (rects/rrects/ovals; fills and fill types;
strokes/hairlines/caps; dashing; clips; blend modes; vertices/patches/atlas; pictures; sprites and
bitmaps generated in code), port them 1:1, and flip entries when all six tiers match (natively or
by model). GMs that need text, codecs, image filters or non-solid shaders stay for Phases 3–5.
Benches (`DEF_BENCH`) for RP, blitters and scan conversion are ported in the same wave to switch
on the perf gate.

### Dependency summary

```
A1 ─┬─ A2a ─┬─ A2b, A2c ───────────────┐
    │       └─ A3 ─┬─ A4 ─┬─ D2 ─┐      │
    ├─ A2d          ├─ A5   │      ├─ D3 ─ D4 ─┐
    └─ A6, A7       └─ B1..B7 (parallel) ┘      ├─ D5 ─ D6 ─┬─ D7
P1 ─ C1 ─┬─ C2, C3, C4 (need D1) ─ C5 ──────────┘           ├─ D8 (after A4)
         └─ C6 ─ C7                                          └─ Wave E
```

---

## 6. Risks and open questions

| # | Risk / question | Recommendation |
|---|---|---|
| R1 | **Vendor-specific estimates** make some Sse2/Sse41/Ml3 goldens unreproducible on Intel runners (Ml4 is expected to be portable, R2). | Fingerprint every host; emulate the oracle host's tables in tests (§4.6); never mark an estimate-dependent GM passing without a matching fingerprint or emulation. Consider Intel goldens later. |
| R2 | ~~No software model for `rcp14`/`rsqrt14`~~ **Resolved (A2d).** | Intel's "Reference Implementations for IA Approximation Instructions VRCP14, VRSQRT14, VRCP28, VRSQRT28, and VEXP2" (`RECIP14.c`, `RCP14S`/`RSQRT14S`): linear interpolation on 64 segments (`rcp14`, input truncated to 16 mantissa bits, `a − 2⁸·b·(x − y)`) or 32 + 32 (`rsqrt14`, 15 bits, slope scale 2⁸ on [1,2) and 2⁷ on [2,4)), evaluated exactly in `f64`, truncated to 16 fraction bits (`& ~(2³⁶−1)`), scaled by 2⁻¹⁸/2⁻¹⁹; exact power-of-two case; denormal inputs normalized (`≤ 0x00200000` → ±∞); denormal results by right shift. `estimates::recip14` implements that structure with coefficients **fitted from the Zen 4 host** (each segment's pair is the unique integer solution reproducing all 1024 host outputs of the segment); they equal Intel's published `RCP14_Coeff`/`RSQRT14_Coeff` (all 256 numbers compared with the published file). Exhaustive check vs Zen 4: **0 mismatches** over 2³² inputs for both. We did not compile Intel's C code itself; the special-value paths were modelled from Zen 4 measurements and agree with the reference's described behaviour. Remaining: run the cheap fingerprint check (`cargo xtask cpu-probe`) on one Intel AVX-512 runner to confirm Intel silicon matches its own reference. |
| R3 | **Interpreter performance** vs Skia's register-passing tail calls. | Bench in A3 before Wave B scales out; fused programs for hot sequences; keep stage bodies small and `#[inline]`; profile LLVM's inlining in the giant `match`. |
| R4 | **Compile time / code size**: 6 native + 5 model instantiations of ~7k lines of stage code. | Models behind `cfg(any(test, feature = "models"))`; native tiers by `target_arch`; measure in A3. If needed, split highp into per-file sub-modules so codegen units parallelize. |
| R5 | **Wasm oracle missing**; Emscripten libm (musl) and `(int)` conversions (`i32.trunc_sat` with nontrapping-fptoint, the default in current LLVM) differ from Windows. | `x64-scalar` proxy now (§4.5); the real wasm oracle before Scalar GMs are marked passing. Rust's `as` saturates like `trunc_sat`. |
| R6 | **Neon oracle missing.** | Build DM on a hosted arm64 runner (§4.5). Neon model lets CI test Neon logic earlier. |
| R7 | **A fifth x86 behaviour** appears in configs or GMs not yet in the suite (e.g. `gray8`, `srgb`, `rec2020`, SkSL-heavy GMs that hit `div_fn` u32 or `floor_` edge cases). | `xtask oracle check-classes` on every publish; rp-diff covers the primitives regardless of GM coverage. |
| R8 | **Stale tail lanes** and scratch persistence look like dead behaviour and may get "simplified". | Replicate exactly (§1.7); a dedicated test with a compiled pipeline run twice, and an SkSL `any`-branch case in rp-diff. |
| R9 | **SSSE3 vs SSE2 `S32_alpha_D32_filter_DX`** both map to `Sse2`; equality is only measured on the GM suite. | Port the SSSE3 variant for `Sse2` (it is what any real SSE2-baseline Skia runs on hardware since 2006) and add rp-diff-style cases comparing both C++ variants (Phase 3). |
| R10 | **Denormal/rounding mode state**: Skia never touches MXCSR/FPCR; a host process that sets FTZ/DAZ changes Skia's and our results alike. | Document; tests assert default MXCSR at start. |
| R11 | **Core/raster boundary**: `Canvas` needs a device; shaders/color filters need `append_stages`. | `Device` and `ShaderBase`-style traits in core, implementations in raster/effects; RP types in simd so core can name them (§2.1). Revisit if `skia-safe` paths force otherwise. |
| R12 | **`SkVx` claims of exactness** rely on Skia's x86/NEON fast paths matching portable code; `if_then_else` with `blendv` uses only sign bits. | Already covered by the vx port's tests; any `SkVx` mask must be canonical (all-ones/zero), which `SkVx` comparisons guarantee. |
| R13 | **PLAN/UNSAFE wording**: PLAN §5.1/§6.3 name `WasmSimd128`/`Hsw`/`Skx` and a SIMD wasm tier. | Follow-up docs PR: rename to the §2.2 tiers and state that Skia's wasm raster pipeline is scalar. |

Open questions for the coordinator:

1. Is golden selection by estimate fingerprint (R1, option 6) worth an Intel oracle host, or do we
   standardize on AMD-generated goldens plus emulation? (Recommendation: emulation first.)
2. Should `skia-rust-raster` own `Surface::new_raster` (cleaner layering) or should core
   (closer to Skia's file layout)? (Recommendation: raster crate, re-exported by the facade.)
3. Do we port Skia's `SkRasterPipelineBench`/`SkBlitterBench` in A3 so the perf gate exists before
   Wave B? (Recommendation: yes, the interpreter design depends on it.)

---

## Appendix: data behind §1.4 and §1.9

- Golden equivalence (m156, 2,727 results/tier): sse2 vs sse41 differ in 52; sse2 vs ml3 in 1,020
  (f16 795, 8888 125, 565 100); sse2 vs ml4 in 1,020; ml3 vs ml4 in 104; ml3 ≡ v3 (0); ml4 ≡ v4 (0);
  sse41 ≡ avx (0); 1,701 identical across all four.
- Estimate probe (Zen 4, rustc 1.99): see §1.4. The probe and the golden-comparison script are
  throwaway and not committed; A1/A2d re-implement them as `cargo xtask cpu-probe`.
- `rcp14`/`rsqrt14` (A2d): outputs over [1,2) and [2,4) are constant on blocks of 2⁷ (`rcp14`)
  and 2⁸ (`rsqrt14`) mantissas apart from mantissa 0; the outputs have ≥ 7 trailing zero bits.
  Fitting `trunc17(S·a − b·(k − 512))` per segment (k = the 10 bits below the segment index)
  gave exactly one integer pair per segment, for all 64 + 32 + 32 segments.

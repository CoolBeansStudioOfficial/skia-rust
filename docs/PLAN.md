# skia-rust — Project Plan

A faithful Rust port of [Skia](https://skia.org) in safe, idiomatic Rust, with a CPU raster backend and a GPU backend modeled on Graphite running on [wgpu](https://wgpu.rs). Success is measured by one number: **the share of Skia's own tests that pass, pixel-for-pixel against real Skia.**

---

## 1. Goals and non-goals

**Goals**
1. **100% of Skia's tests**, in every in-scope module, matched **exactly** against a pinned Skia release: every CPU tier and the reference GPU environments.
2. **Skia's API, idiomatic Rust.** Same concepts and names (without the `Sk` prefix), Rust ownership, `Option`/`Result`, enums, builders.
3. **wgpu GPU backend** that ports Graphite's architecture and matches Skia Graphite-on-Dawn exactly.
4. **Safe Rust.** `unsafe` only in one SIMD crate, under strict rules.
5. **Performance parity:** each benchmark equal to or faster than Skia.

**Non-goals**
- Binding to Skia or linking C/C++ at runtime. Skia is used **only** as a test oracle.
- Ganesh (Skia's legacy GPU backend). Graphite only.
- Tests for Skia internals that have no public-API equivalent in Rust (e.g. `SkTDArray`). These are excluded **individually, with a written reason**, and the exclusion list is published.

---

## 2. Decision log

| Area | Decision |
|---|---|
| Name | `skia-rust` (crates.io name confirmed available) |
| License | BSD-3-Clause (same as Skia); Skia's copyright notice is kept for ported code |
| Edition / MSRV | Edition 2024; MSRV = latest stable |
| Layout | Cargo workspace of internal crates + `skia-rust` facade crate with feature flags |
| Skia pin | Latest Skia release (`chrome/mNNN` branch), upgraded **every milestone** |
| Implementation style | Faithful function-by-function port: same arithmetic and operation order, written as idiomatic Rust, each item linked to its C++ source |
| API naming | Drop `Sk` prefix, Rust casing (`SkCanvas::drawRect` → `Canvas::draw_rect`), `#[doc(alias = "SkCanvas")]` on everything |
| Unsafe | Workspace `deny(unsafe_code)`; allowed **only** in `skia-rust-simd`, with documented, minimal, Miri-checked blocks |
| Lints | `clippy::pedantic` (warn, CI treats warnings as errors) + `cargo deny` |
| Test scope | Everything: core, effects, CPU raster, PathOps, GPU (Graphite), text, codecs, skcms, PDF, SVG, Skottie, Paragraph |
| Test porting | Faithful 1:1 translation of Skia's tests and GMs |
| CPU exactness | Match **every** Skia CPU tier exactly |
| GPU oracle | Skia Graphite + Dawn (custom build at the pin) |
| GPU reference environments | Vulkan/lavapipe and D3D12/WARP gate merges; real hardware (RTX 4070 SUPER) is report-only |
| Text | Fontations (`skrifa`); the oracle uses Skia's Fontations typeface backend |
| Shaping / Unicode | HarfRust + ICU4X |
| SkSL | Full port of Skia's SkSL compiler, including the WGSL and Raster Pipeline code generators |
| Codecs | Faithful Rust ports of the decode paths of the libraries Skia wraps (libjpeg-turbo, libwebp, Wuffs GIF, libpng / Rust `png` as Skia configures it), incl. JPEG gainmaps. Scope = codecs enabled in Skia's default GN build at the pin; others (AVIF, JPEG XL, RAW) excluded with reason until revisited |
| Goldens | Generated on the local server (Windows, Ryzen 7 7800X3D, RTX 4070 SUPER) and **published** as release artifacts |
| CI platforms | Windows x64, Linux x64, macOS arm64, Linux aarch64, wasm32 |
| wasm oracle | Skia built with Emscripten (CanvasKit-style) at the **simd128** tier, run under Node; matched exactly |
| Perf | Gate: ≤ 0% slower than Skia per bench (measured on the server); GPU benches compare against the wgpu-capability-restricted oracle |
| GPU perf features | Async pipeline compilation (background threads) + wgpu `PipelineCache`. Dawn-only tile-GPU extensions (transient attachments, MSAA render-to-single-sampled, load-resolve, framebuffer fetch) are not pursued; desktop parity is the target |
| Agents | Cheapest model first, escalating on failure: Haiku 4.5 → Sonnet 5.5 → Opus 5.5; wide parallel fan-out allowed (§8.3) |
| Workflow | Inventory-driven, small PRs (one manifest entry or a small batch per PR) |
| Publishing | Reserve the name now; first real release when core + CPU raster are at 100% |
| Versioning | `0.NNN.patch`, where NNN = the Skia milestone being matched |

---

## 3. Architecture

### 3.1 Workspace layout

```
skia-rust/
├─ Cargo.toml                 # workspace, shared lints, shared deps
├─ CLAUDE.md                  # porting rules for agents (see §8)
├─ crates/
│  ├─ skia-rust/              # facade: re-exports, feature flags
│  ├─ skia-rust-simd/         # the ONLY crate allowed to use `unsafe`; SkVx equivalent + per-tier kernels
│  ├─ skia-rust-core/         # scalars, Point/Rect/RRect, Matrix/M44, Path/PathBuilder, Color, ImageInfo,
│  │                          #   Pixmap/Bitmap/Image, Paint, Canvas, clip stack, Picture, Surface
│  ├─ skia-rust-raster/       # scan conversion (non-AA, supersampled, analytic AA), blitters,
│  │                          #   Raster Pipeline (highp/lowp), stroker, dashing
│  ├─ skia-rust-skcms/        # skcms port (color management)
│  ├─ skia-rust-effects/      # shaders, gradients, color/mask/image filters, path effects, blenders
│  ├─ skia-rust-sksl/         # SkSL front end, IR, optimizer; RP + WGSL code generators
│  ├─ skia-rust-pathops/      # boolean path ops
│  ├─ skia-rust-text/         # Typeface/Font (Fontations), strikes, glyph cache, TextBlob, test typefaces
│  ├─ skia-rust-codec/        # Codec API + faithful ports: libjpeg-turbo, libwebp, Wuffs GIF, PNG, BMP/ICO/WBMP, gainmaps
│  ├─ skia-rust-gpu/          # Graphite port on wgpu: Context, Recorder, Recording, DrawPass, renderers, atlases
│  ├─ skia-rust-shaper/       # SkShaper on HarfRust
│  ├─ skia-rust-unicode/      # SkUnicode on ICU4X
│  ├─ skia-rust-paragraph/    # skparagraph
│  ├─ skia-rust-svg/          # SVG DOM + SVG canvas
│  ├─ skia-rust-skottie/      # Lottie player
│  └─ skia-rust-pdf/          # PDF backend
├─ tests/                     # publish = false
│  ├─ unit/                   # 1:1 ports of skia/tests/*.cpp (DEF_TEST, DEF_GRAPHITE_TEST…)
│  ├─ gm/                     # 1:1 ports of skia/gm/*.cpp (DEF_GM, DEF_SIMPLE_GM…)
│  └─ modules/                # ports of modules/*/tests
├─ benches/                   # 1:1 ports of skia/bench (DEF_BENCH), harness compares against Skia
├─ fuzz/                      # cargo-fuzz targets mirroring skia/fuzz
├─ oracle/                    # Skia build scripts + C++ dump tool (never shipped)
├─ inventory/                 # manifest generator + manifest.toml
├─ xtask/                     # cargo xtask: oracle, goldens, test, bench, inventory, bump
└─ docs/                      # PLAN.md, PORTING.md, API_MAPPING.md, UNSAFE.md, EXCLUSIONS.md
```

**Layering** (enforced by crate dependencies): `simd` → `core` → `skcms` / `raster` → `effects` / `sksl` / `pathops` → `text` / `codec` → `gpu` → `shaper` / `unicode` → `paragraph` / `svg` / `skottie` / `pdf` → facade. No crate may depend on a crate above it.

### 3.2 Facade features
`default = ["raster", "effects", "pathops", "codec", "text"]`; opt-in: `gpu`, `sksl` (pulled in by `gpu` / runtime effects), `paragraph`, `svg`, `skottie`, `pdf`.

### 3.3 API conventions (detailed in `docs/API_MAPPING.md`)
- `sk_sp<T>` → `Arc<T>` for immutable shared objects (`Shader`, `Image`, `Typeface`, …); plain values for value types (`Paint`, `Path`, `Matrix`).
- A nullptr return becomes `Option<T>`, or `Result<T, Error>` where Skia reports a reason.
- Integer flags become `bitflags!` types; C++ enums become Rust `enum`s with Skia's discriminants preserved.
- `Canvas` borrows its `Surface` (`surface.canvas()` → `&mut Canvas<'_>`).
- Builders where Skia uses many optional parameters (`ImageFilter`s, `Paragraph` styles).
- Method names: `snake_case` of the Skia name; overloads are disambiguated by suffix (`draw_image` / `draw_image_rect`) following a fixed rule list.

---

## 4. Unsafe and SIMD policy (`docs/UNSAFE.md`)

- Workspace: `[workspace.lints.rust] unsafe_code = "deny"`. Only `skia-rust-simd` sets `unsafe_code = "allow"`; CI greps that no other crate overrides it.
- Inside `skia-rust-simd`:
  - `unsafe_op_in_unsafe_fn = "deny"`, `clippy::undocumented_unsafe_blocks = "deny"`, `clippy::multiple_unsafe_ops_per_block = "deny"`, `clippy::missing_safety_doc = "deny"`.
  - Each `unsafe` block holds **one operation** and carries a `// SAFETY:` comment.
  - Prefer safe `#[target_feature]` functions (Rust ≥ 1.86). `unsafe` exists only where unavoidable: calling a tier function after runtime detection, unaligned loads/stores.
  - Every SIMD kernel has a **portable scalar twin**. Tests assert the two are bit-identical on random inputs (proptest) and on every golden.
  - Miri runs over the scalar paths and the safe wrappers on every PR.
- The public API of `skia-rust-simd` is entirely safe. Other crates never see a raw pointer.
- Red flags that block review: `unsafe` used to skip bounds checks without a benchmark showing it matters, `transmute`, `static mut`, lifetime laundering.

---

## 5. The oracle

Real Skia, built from source at the pin, is the single source of truth. It is test tooling only and never a dependency of a published crate.

### 5.1 Builds (on the server, driven by `cargo xtask oracle build`)
- Source: `chrome/mNNN` branch of Skia, synced with `tools/git-sync-deps`.
- GN args (common): release, non-official (bundled third-party libs, tools enabled), clang-cl on Windows (the compiler Chrome ships Skia with), test-font manager, no system fonts. Graphite + Dawn and Fontations in the builds that need them. Exact args live in `oracle/tiers.toml`.
- **CPU builds, one per compile-time x86 baseline** (`x64-sse2`, `x64-ssse3`, `x64-sse41`, `x64-sse42`, `x64-avx`, `x64-v3`, `x64-v4`). Each build also runs at every runtime `SkOpts` level above its baseline (ssse3, ml3 = x86-64-v3, ml4 = x86-64-v4), selected by `SKIA_ORACLE_CPU_CAP`, a small oracle-side patch to `SkCpu`. A tier is one (build, runtime level) pair; xtask derives the full list (19 x64 tiers at m156). The 7800X3D (Zen 4) runs all of them natively, and each run verifies the tier that actually executed.
  - `arm64-neon`: built and run on macOS arm64 / Linux aarch64 hosted runners. The first time, goldens are generated there and uploaded.
  - `wasm-simd128`: Skia built with Emscripten the way CanvasKit is (`-msimd128`), with `oracle-dump` compiled to wasm and run under Node. The reference for our wasm32 target, whose `Tier::WasmSimd128` must match it exactly. No non-SIMD wasm tier.
  - Raster Pipeline **highp and lowp** are both exercised; the dump tool records which pipeline each draw used.
- **GPU builds:** Graphite + Dawn on D3D12 and on Vulkan.
  - Dawn toggles and Graphite caps are restricted to **wgpu's feature set** (e.g. no Dawn-only extensions), so both sides take the same Graphite code paths.
- Environments: D3D12/WARP (Windows host), Vulkan/lavapipe (WSL2 on the server), plus D3D12 and Vulkan on the RTX 4070 SUPER (report only).

### 5.2 The dump tool: DM + `OracleDump`
- The oracle is Skia's own test runner **DM**, so sources, configs and rendering are exactly what Skia's bots run. `oracle/dm/OracleDump.cpp` (copied in by `cargo xtask oracle patch`) adds `--oracleRawPath`; see `oracle/README.md`.
- Inputs: DM's own `--src`, `--config`, `--match`, plus the tier (runtime CPU cap).
- Outputs per item: raw pixel buffer (exact bytes) or encoded bytes for PDF/SVG/SKP sinks, and JSON metadata (size, color type, alpha type, serialized color space, CPU tier that ran). xtask computes the SHA-256 hashes and records the compiler version and GN args in `toolchain.txt`.
- **Debug modes** for diagnosing mismatches: dump the Raster Pipeline stage list, generated SkSL/WGSL text, path verbs after transforms, Graphite draw-pass/renderer choices, glyph-cache contents.
- Color configs mirror Skia DM's: `8888`, `565`, `gray8`, `f16`, `srgb`, `rec2020`, and so on. In-scope configs are listed in the manifest.

### 5.3 Golden store and publishing
- Content-addressed on the server: `goldens/<skia-commit>/<config>/<test-id>.{raw,png,json}`.
- `cargo xtask goldens publish` uploads, per pin:
  - **`hashes-mNNN.json`**: the SHA-256 of every (test, config). This is small, and it's all CI needs to check an exact match.
  - Per-config tarballs of PNGs and raw buffers as GitHub Release assets (`goldens-mNNN`), downloaded **only on a mismatch** to produce a diff image.
- The hash file is checksummed and committed to the repo (`inventory/goldens.lock`), so every commit names the exact goldens it's tested against.

---

## 6. Testing

### 6.1 Test inventory (`inventory/`)
- `cargo xtask inventory sync` parses the pinned Skia tree for `DEF_TEST`, `DEF_GRAPHITE_TEST*`, `DEF_GANESH_TEST*` (recorded, excluded by policy), `DEF_GM`, `DEF_SIMPLE_GM*`, GM subclasses, `DEF_BENCH`, module tests and fuzzers.
- `inventory/manifest.toml` holds one entry per item:
  ```toml
  [[test]]
  id      = "gm/strokes.cpp::strokes_round"
  kind    = "gm"
  source  = "gm/strokes.cpp#L42-L97"
  hash    = "sha256 of the C++ source span"     # detects upstream changes on bumps
  module  = "raster"
  status  = "passing"                           # todo | ported | failing | passing | excluded
  configs = ["cpu-*-8888", "cpu-*-f16", "gpu-dawn-*-8888"]
  reason  = ""                                  # required for excluded / failing
  ```
- **The published metric** is `passing / (total − excluded)`, per module, per config and overall. It's rendered into the README and a dashboard. `docs/EXCLUSIONS.md` is generated from the manifest.

### 6.2 What "passing" means
| Kind | Pass condition |
|---|---|
| Unit test (`DEF_TEST`) | The ported assertions all hold. Assertions may never be loosened relative to the C++. |
| GM, CPU | SHA-256 of our raw output == the oracle's, **for every CPU tier** in its configs |
| GM, GPU | SHA-256 of our raw output == the oracle's on **lavapipe and WARP** |
| GM, real HW | Reported (diff count, max delta), never gating |
| Codec / PDF / SVG / Skottie | Same rules, applied to decoded pixels or rasterized output; byte-identical files where Skia's tests compare bytes |

There are **no tolerances** anywhere. A mismatch is a bug, or an exclusion with a written reason.

### 6.3 Tier coverage in our code
- `skia-rust-simd` exposes `Tier` (`Portable`, `Sse2`, `Sse41`, `Avx`, `Hsw`, `Skx`, `Neon`, …). Runtime dispatch is the default; tests use `force_tier(Tier)` (test-only API) to run every tier the host supports.
- Each GM runs once per supported tier and is compared to that tier's golden.

### 6.4 Other test layers
- **Proptest:** random paths, matrices, paints. Checked for SIMD-vs-scalar equality, plus invariants (e.g. path bounds contain all points).
- **Fuzzing:** `cargo-fuzz` ports of `skia/fuzz` targets (PathOps, SkSL, codecs, region, path deserialization), run nightly on the server. Crashes become regression tests.
- **Miri:** `skia-rust-simd` scalar paths and safe wrappers, on every PR.
- **Shader-level differential tests** (GPU): every SkSL module/snippet goes through our SkSL → WGSL and Skia's. The generated **WGSL text must be identical**, and each shader is executed on both Dawn and wgpu with recorded inputs. This catches divergence before it shows up as a pixel diff.
- **Doc tests** on every public item.

### 6.5 Running tests
```
cargo xtask test                       # unit tests + CPU GMs for tiers this host supports
cargo xtask test --gpu lavapipe        # GPU GMs on a reference environment
cargo xtask test --only gm/strokes     # filter
cargo xtask diff gm/strokes::strokes_round --config cpu-x64-hsw-8888   # fetch golden, write diff image
```

---

## 7. CI (GitHub Actions)

| Job | Runners | Gates? |
|---|---|---|
| fmt, clippy (pedantic, `-D warnings`), `cargo deny`, unsafe-policy check | Linux x64 | yes |
| Build + unit tests | Windows x64, Linux x64, macOS arm64, Linux aarch64 | yes |
| wasm32 (`+simd128`) build + CPU tests under wasmtime vs `wasm-simd128` goldens | Linux x64 | yes |
| CPU GMs vs published hashes, all tiers the runner supports; SKX via Intel SDE | Windows x64, Linux x64, macOS arm64, Linux aarch64 | yes |
| GPU GMs on lavapipe | Linux x64 (Mesa lavapipe) | yes* |
| GPU GMs on WARP | Windows x64 | yes* |
| Miri | Linux x64 | yes |
| Manifest consistency: no `passing → failing` regression; every `passing` entry actually ran; no assertion loosening (diff check on ported tests vs C++ hash) | Linux x64 | yes |
| Fuzz (short run) | Linux x64 | nightly, non-gating |

\* Gating only if Phase 0 shows lavapipe/WARP output is identical across host CPUs. If it isn't, GPU exact-match gating moves to the server (§9.2), and hosted runners report only.

**Server-only checks** (run with `cargo xtask server-check`; the result is posted to the PR as a commit status by the script):
- Perf gate (§9.2).
- Real-hardware GPU report (RTX 4070 SUPER, D3D12 + Vulkan).
- Golden regeneration on pin bumps.

---

## 8. Development workflow

### 8.1 `CLAUDE.md` porting rules (abridged)
1. Pick work from the manifest (`status = "todo"`). One entry, or a small cohesive batch, per PR.
2. **Port, don't invent.** Every ported function starts with `// Port of: src/core/SkScan_AntiPath.cpp@<commit>#L120-L180`.
3. Keep Skia's arithmetic and evaluation order exactly: same `f32`/`f64`/fixed-point types, same rounding, `mul_add` **only** where Skia uses FMA (SkVx `fma`), no reassociation, no "simplifications" of formulas.
4. Idiomatic Rust *around* the arithmetic: ownership, enums, iterators, `Option`/`Result`, no global mutable state.
5. No `unsafe` outside `skia-rust-simd`. No new `allow` of clippy lints without a comment explaining why.
6. Ported tests are 1:1. Never weaken an assertion, never `#[ignore]` without changing the manifest status and giving a reason.
7. A PR is done when: the manifest entries flip to `passing`, CI is green, and nothing previously passing regressed.
8. When output mismatches, use the oracle debug dumps (§5.2) before changing code.

### 8.2 Branching and review
- `main` is protected; it requires all gating checks plus the server-check status for PRs touching benched code.
- Squash merges; PR titles reference manifest IDs.
- Pin bumps happen on `bump/mNNN` branches (§9.1).

### 8.3 Agent staffing and escalation
- **Model ladder: cheapest first.** Every manifest task starts on Haiku 4.5. On failure it moves up one rung: Haiku 4.5 → Sonnet 5.5 → Opus 5.5.
- **Escalation triggers** (any one moves the task up a rung):
  - the manifest entry is still not `passing` after **2 full attempts** (each = port, run the oracle comparison, apply a debug-dump-driven fix);
  - a `passing → failing` regression the agent can't root-cause in one attempt;
  - the CI unsafe-policy or Miri checks fail in `skia-rust-simd`.
- **Escalation handoff:** the failing agent writes `notes/<manifest-id>.md` with what it tried, the diff images/hashes, and the relevant oracle dumps. The next rung starts from that file, not from scratch. Notes stay in the repo as a record of hard cases.
- **Opus from the start**, limited to work where a wrong design is expensive:
  - per-module design notes before porting starts (e.g. how Graphite's `Recorder`/`DrawPass` map onto wgpu and Rust ownership);
  - review of every `unsafe` block added to `skia-rust-simd`;
  - pin-bump triage when a milestone changes a ported region structurally.
- **Parallelism:** independent manifest entries are worked by many agents at once (workflow fan-out), each in its own git worktree and branch.
- CI is the reviewer for ordinary PRs. Exact-match hashes and the manifest checks are strict enough that a model review adds little. PRs are squash-merged once CI is green.
- Track escalation rate per module and per rung. A module with frequent escalations gets an Opus-written design note so later tasks succeed on a cheaper rung.

### 8.4 Docs
- `docs/PORTING.md`: the full rule set, Skia → Rust idiom cookbook.
- `docs/API_MAPPING.md`: every public Skia symbol → Rust symbol (generated, with a hand-written rationale for non-obvious mappings).
- rustdoc on all public items, with `#[doc(alias)]` for Skia names and links to Skia's docs.

---

## 9. Maintenance

### 9.1 Milestone upgrades (every Chrome milestone, ~4 weeks)
1. A scheduled workflow detects a new `chrome/mNNN` branch and opens a tracking issue.
2. Server: `cargo xtask bump mNNN`
   - builds all oracle configs at the new pin,
   - regenerates goldens and publishes `goldens-mNNN`,
   - re-syncs the inventory. New tests become `todo`. Tests whose C++ source hash changed become `stale`. Removed tests are deleted.
   - For every `// Port of:` link, diffs the upstream source span between the old and new pins and files one issue per changed ported region.
3. Work happens on `bump/mNNN` until there are no `passing → failing` regressions, then it merges and `0.NNN.0` is cut.
4. Exit rule: `main` is never on a pin where it passes less than it did on the previous pin, except for newly added upstream tests.

### 9.2 Performance
- Skia's `bench/` (`DEF_BENCH`) is ported 1:1 into `benches/` and listed in the manifest.
- `cargo xtask bench --vs-skia` runs on the server (Ryzen 7 7800X3D) with a fixed CPU frequency/power plan, pinned cores and the same tier on both sides. It uses repeated runs and compares medians.
- **Gate:** for each bench, our median ≤ Skia's median. A bench fails only when it is slower **and** the difference is statistically significant (Mann-Whitney U, p < 0.05), so noise alone can't fail it.
- GPU benches compare against the oracle built with the **same restricted capabilities** used for pixel tests (§5.1). Otherwise Dawn-only performance extensions (MSAA render-to-single-sampled, transient attachments, framebuffer fetch) would make parity impossible by construction.
- Results are kept as a time series, published to the dashboard, and posted to the PR.

### 9.3 Versioning and releases
- `0.NNN.patch`. NNN = the Skia milestone. A milestone bump is a breaking release; patches are fixes within a pin.
- Now: publish a placeholder `skia-rust 0.0.0` to reserve the name (and the `skia-rust-*` internal crate names as they're created).
- First real release: core + effects + CPU raster at 100% of their in-scope tests on all CPU tiers.
- Each release's notes include the pass-rate table and the perf summary.

### 9.4 Dependencies and security
- `cargo deny`: license allowlist (BSD/MIT/Apache/Zlib/Unicode), advisory DB, duplicate-version check.
- Renovate/Dependabot weekly. wgpu/naga, skrifa, HarfRust and ICU4X bumps run the full GPU and text suites.
- Fuzzing runs nightly on the server; OSS-Fuzz application once the core is stable.

### 9.5 Legal
- `LICENSE`: BSD-3-Clause with both the skia-rust copyright and Google's Skia copyright (for ported code).
- Ported files keep a header noting their origin.
- README states the project is not affiliated with or endorsed by Google (BSD-3 clause 3).

---

## 10. Roadmap

Each phase ends when its exit criteria are met; later phases can start early where dependencies allow.

| Phase | Content | Exit criteria |
|---|---|---|
| **0. Infrastructure** | git + GitHub repo, workspace skeleton, lints, CI matrix, `CLAUDE.md`, `xtask`, oracle builds for all configs on the server, `oracle-dump`, golden publishing, inventory + manifest, dashboard, name reservation | Manifest lists every Skia test/GM/bench at the pin with real counts; goldens published for all configs; a hand-built image round-trips through hash comparison; lavapipe/WARP cross-host determinism measured |
| **1. Foundations** | `skia-rust-simd` (all tiers + scalar twins), scalars/fixed point, Point/Rect/RRect/Region, Matrix/M44, Path/PathBuilder/PathMeasure, Color/Color4f, ImageInfo, Pixmap/Bitmap, skcms | 100% of the corresponding unit tests (MatrixTest, PathTest, RegionTest, RRectTest, skcms tests…) |
| **2. CPU raster** | Raster Pipeline (highp + lowp), blitters, scan converters (non-AA, supersampled, analytic AA), clip stack, Canvas/Surface, Paint, all blend modes, stroker, dashing, basic shaders, Picture record/playback | All CPU GMs in scope that need no effects/text/codecs pass on **every** CPU tier; perf gate active for ported benches |
| **3. Effects + SkSL (CPU)** | Gradients, image shaders, sampling/mipmaps, color filters, mask filters (blur), image filters, path effects, blenders; SkSL front end + Raster Pipeline codegen → RuntimeEffect on CPU | Effects/runtime-effect unit tests + GMs at 100% on all CPU tiers → **first crates.io release** |
| **4. PathOps + codecs** | PathOps; Codec API; faithful ports of libjpeg-turbo (`ISLOW` IDCT, fancy upsampling, color conversion), libwebp (VP8/VP8L), Wuffs GIF, PNG, BMP/ICO/WBMP; JPEG gainmaps | PathOps tests (incl. the large op/simplify suites) and codec tests at 100% |
| **5. Text** | Fontations Typeface, Font, Strike/glyph cache, scaler context, TextBlob, Skia test typefaces (`SkTestTypeface`, SVG/COLR fonts) | All text GMs/tests at 100% on CPU |
| **6. GPU (Graphite on wgpu)** | SkSL → WGSL codegen (byte-identical to Skia), Context/Recorder/Recording, async pipeline compilation + `PipelineCache`, resource cache, DrawPass, renderers (analytic shapes, tessellation, atlas paths, coverage masks), text atlases, compute paths, image/texture uploads | Byte-identical WGSL for all Skia shaders; all Graphite tests and GPU GMs exact on lavapipe + WARP; RTX report published |
| **7. Modules** | SkShaper (HarfRust), SkUnicode (ICU4X), Paragraph, SVG, Skottie, PDF | Module tests at 100% → overall 100% |
| **Ongoing** | Milestone bumps, perf parity, fuzzing, API polish toward 1.0 | — |

---

## 11. Risks and mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| **naga vs Tint** compile identical WGSL differently (FMA contraction, precision, HLSL/SPIR-V lowering), so GPU pixels differ | Blocks exact GPU match | Shader-level differential tests (§6.4); compare the HLSL/SPIR-V naga and Tint emit; fix or upstream naga changes; Dawn toggles that disable contraction; last resort, an exclusion with the root cause written down |
| Dawn exposes features wgpu doesn't, and Graphite picks different paths | Divergent rendering | Oracle caps restricted to wgpu's feature set (§5.1) |
| lavapipe/WARP not deterministic across host CPUs | Can't gate GPU on hosted runners | Measured in Phase 0; fall back to server-only GPU gating |
| Decoder ports are large (libjpeg-turbo, libwebp) and their SIMD paths must match their C paths | Phase 4 slips | Port the C paths first (upstream guarantees SIMD/C equality for the paths Skia uses; verify with the oracle per tier), then add SIMD for perf; port only the decode paths Skia actually calls |
| Emscripten build of Skia + Node runner is another toolchain to maintain | wasm gating fragile | Script in `xtask`, cache by pin; it's the same build CanvasKit uses, so upstream keeps it working |
| PDF output requires zlib-identical deflate | PDF byte tests fail | Use a zlib-compatible Rust deflate (`zlib-rs`) and verify against Skia's zlib configuration; port it if needed |
| ≤ 0% perf gate on safe Rust vs hand-tuned C++ | PRs blocked | Faithful SIMD tiers, profile-guided fixes, statistical gate (§9.2); bounds checks removed by structure, not `unsafe` |
| Monthly Skia churn | Constant rework | Source-span hashes + per-port diff issues (§9.1) make the churn mechanical |
| Scale (thousands of tests, a large SkSL compiler, Graphite) | Slow progress | Inventory-driven small PRs, an always-visible pass-rate, strict phase order |
| Matching every CPU tier (incl. NEON, AVX-512, future tiers) | Lots of extra kernels | Scalar twins + a shared SkVx-style abstraction; SKX native on Zen 4; NEON on hosted ARM runners |
| Oracle build complexity on Windows (Skia + Dawn + WSL2 for lavapipe) | Phase 0 slips | Script everything in `xtask`; cache builds keyed by pin + GN args |

---

## 12. Open items for Phase 0
- ~~Confirm Skia exposes a usable switch for forcing SkOpts tiers at runtime~~: it doesn't; `oracle/patches/skia-oracle.patch` adds `SKIA_ORACLE_CPU_CAP`.
- Fontations in Skia's GN build compiles Rust through Bazel; confirm that works on Windows, or fall back to a Linux (WSL2) text oracle.
- ~~Dawn adapter selection~~: `SKIA_ORACLE_DAWN_ADAPTER` in the oracle patch.
- Linux/WSL2 oracle build for lavapipe; WARP determinism across hosts (it is deterministic run-to-run on the server).
- Confirm Skia's Fontations backend covers everything the test font manager needs (and which test typefaces bypass it).
- Decide the exact list of DM color configs in scope (start: `8888`, `f16`, `565`, `gray8`, `srgb`).
- Reference hardware for any future tile-GPU feature paths: deferred until/unless those features are pursued.
- Get the real test/GM/bench counts from the inventory and turn the phase table into dated milestones.

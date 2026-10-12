# The oracle

Real Skia, built from the pinned checkout (`inventory/skia-pin.toml`), is the reference every skia-rust output is compared against. It is test tooling only and never a dependency of a published crate. See `docs/PLAN.md` §5.

## How it works

The oracle **is Skia's own test runner, DM**, with one small addition. DM already knows every GM, image, Skottie and SVG source and every color config, and it renders them exactly the way Skia's bots do. We don't reimplement any of that. We add:

| File | What it does |
|---|---|
| `dm/OracleDump.{h,cpp}` | Adds `--oracleRawPath <dir>` to DM. For every result it writes the exact bytes: `.raw` for pixels (rows tightly packed, `minRowBytes` each) or `.bin` for encoded output (PDF, SVG, SKP), plus a `.json` with size, color type, alpha type, serialized color space and the CPU tier that actually ran. |
| `patches/skia-oracle.patch` | Hooks `OracleDump` into DM's result path and adds it to the `dm` GN target. Adds `SKIA_ORACLE_CPU_CAP` to `SkCpu`, which hides runtime CPU features above a level so one binary can produce every runtime tier. Adds `SKIA_ORACLE_DAWN_ADAPTER` to Graphite's Dawn test context, which picks the adapter by name (WARP, lavapipe, a real GPU) and fails rather than falling back. |
| `tiers.toml` | The expected tier equivalence classes, and the GN builds (one per compile-time x86 baseline, plus Graphite+Dawn) from which xtask derives CPU tiers. |

`cargo xtask oracle patch` copies `dm/` into Skia and applies the patch (idempotent). On a pin bump, if the patch no longer applies, regenerate it against the new milestone; it is small on purpose.

## CPU tiers

Skia picks x86 code in two places: the compile-time baseline (`SK_CPU_X64_LEVEL`) and runtime `SkOpts` dispatch (ssse3, ml3 = x86-64-v3, ml4 = x86-64-v4). A tier is one (build, runtime level) pair, and xtask derives every pair a shipped Skia can run. `cargo xtask oracle tiers` lists them.

**RGBA variants.** The default builds run on Windows, where Skia's N32 is BGRA, so their `8888` goldens are `BGRA_8888`. On macOS/Linux N32 is RGBA, so three more builds (`x64-sse2-rgba`, `x64-sse41-rgba`, `x64-scalar-rgba`) add `-DSK_R32_SHIFT=0` (a define only; no Skia source changes). Only one build per behaviour class is built, and a build's `runtime_levels` limits which runtime tiers are derived, giving five tiers: `cpu-x64-sse2-rgba` (+ `-rt-ml3`, `-rt-ml4`), `cpu-x64-sse41-rgba` and `cpu-x64-scalar-rgba`. Render only `8888` for them (`565` and `f16` are byte-order independent): `cargo xtask oracle run cpu-x64-sse2-rgba --config 8888 --src gm --fresh`. They form their own `[[class]]`es, parallel to the BGRA ones.

Each run checks that every result actually ran at the requested level. If the host CPU lacks a feature, for example AVX-512 for `ml4`, the run is rejected rather than silently producing a lower tier's output. The Ryzen 7 7800X3D (Zen 4) runs every x86 tier natively.

All builds use clang-cl, the compiler Chrome ships Skia with. Every run records the compiler version and GN args in `toolchain.txt` next to the goldens, because a different compiler can change floating-point codegen (FMA contraction) and therefore pixels.

## Fonts

Runs use `--nativeFonts false`, i.e. Skia's portable test font manager, which is pure Skia and identical on every platform (and is what Skia's default bots run). That manager returns null for every font file, so GMs that load real fonts (`ToolUtils::CreateTypefaceFromResource`) skipped or drew with the empty typeface; no golden contains Fontations, FreeType or DirectWrite output. Text goldens are therefore trustworthy for the portable typefaces; Fontations pixels need a separate oracle configuration (`docs/design/text.md` §1, §7.3).

## CI-hosted oracle (CPU)

The Windows server is gone. Since 2026-10-12 the CPU oracle also runs on GitHub's hosted runners: `.github/workflows/oracle.yml` (`workflow_dispatch` only; it never runs on a PR or push) builds Skia at the pinned revision on `ubuntu-latest` with clang, renders with the same DM and the same patch, and uploads goldens-format hash files as workflow artifacts. Nothing is published from CI: the maintainer downloads the artifacts and publishes them as release assets. Design: `docs/design/gpu.md` §3.1 (Q1), `modules.md` Q6, `codecs.md` Q2 (b), `bench.md` Q1 and §5.2.

```
gh api -X POST repos/<owner>/skia-rust/actions/workflows/oracle.yml/dispatches -f ref=<branch>
# optional inputs: builds (JSON list), extras, lottie_configs, nanobench, libm_shim, rebuild, goldens_release
gh run download <run id> -n oracle-dm-hashes-m156      # the image and lottie hashes
```

A GitHub quirk: a workflow file must exist on the default branch before it can be dispatched by name, unless a push has registered it once. To dispatch from a branch before the first merge, push the branch with a temporary `push:` trigger for a throwaway branch name, then remove the trigger.

### Jobs

| Job | What it does |
|---|---|
| `build` (one per build: `x64-sse2-rgba`, `x64-sse41-rgba`, `x64-scalar-rgba`) | Clones Skia at the pin, syncs its dependencies, copies `oracle/dm/*`, applies `patches/skia-oracle.patch`, `gn gen` with the args of `tiers.toml`, `ninja dm nanobench`. Caches `dm`, `nanobench`, `args.gn`, `toolchain.txt` and `resources/` (key: pin + GN args + patch + `oracle/dm` + the nanobench switch). The Skia checkout with its dependencies is cached separately (key: pin). A cold build takes about 40 minutes on the 4-vCPU runner (`dm` 17, nanobench 1 more); a cache hit takes seconds. |
| `render` (one per tier: `cpu-x64-sse2-rgba`, `-rt-ml3`, `-rt-ml4`, `cpu-x64-sse41-rgba`, `cpu-x64-scalar-rgba`) | Restores the binaries and renders `--src gm --config 8888` with `SKIA_ORACLE_CPU_CAP`, compares every hash with the published tier of the same name (`oracle/ci/oracle_ci.py compare`: counts, and a class for every mismatch computed from the golden objects), then (`extras`) renders `--src image` (`8888 565 f16` over `resources/images`) and `--src lottie` (`resources/skottie`). Takes about 5 minutes. Uploads `oracle-results-<tier>`. |
| `collect` | Merges the image and lottie hashes of all tiers into `oracle-dm-hashes-m156`. |

### Build

`oracle/ci/oracle_ci.py gn-args <build>` turns `tiers.toml` into Linux args: the shared args, `extra_cflags` with the `/clang:` prefixes removed (`-ffp-contract=off`, `-msse4.1`, `-DSK_R32_SHIFT=0`, `-DSKRP_CPU_SCALAR`) and `cc="clang"`. Two args differ from Skia's Linux defaults: `skia_use_freetype=false` and `skia_use_fontconfig=false`, because Skia enables both only on Linux and the Windows oracle has neither (the runs use the portable test font manager either way). The compiler is the runner's `clang` (18.1.3, libstdc++, glibc 2.39 at the time of writing); the Windows oracle used clang-cl 23.1.2. `toolchain.txt` in every artifact records compiler, runner image and GN args. partition_alloc stays at Skia's default (on), as on Windows. The compiler is not part of the cache key (a runner image update does not rebuild); pass `rebuild` after one.

### Artifacts

- `oracle-results-<tier>`: `results/<tier>/hashes-gm.json` (our `--src gm` hashes), `equivalence.{md,json}`, `hashes-image.json`, `hashes-lottie.json`, `equivalence-shim.*` (diagnostic, below), the DM logs, `results/cpu.txt` (runner CPU) and `libm-imports.txt`.
- `oracle-dm-hashes-m156`: `dm-image-hashes-m156.json` and `dm-lottie-hashes-m156.json`, in the format of `hashes-m156.json` (`{skia_branch, skia_commit, tiers: {<tier>: {toolchain, results: {<id>: sha256}}}}`; ids are `<config>/<src>/<options>/<name>`, hashes are SHA-256 of the raw bytes DM wrote, as everywhere else), and `equivalence-m156.md`. `8888` results are filed under the RGBA tier names (`cpu-x64-sse2-rgba`, ...). `565` and `f16` do not depend on the N32 byte order, so they are filed under the BGRA tier names (`cpu-x64-sse2`, `-rt-ml3`, `-rt-ml4`, `cpu-x64-sse41`, `cpu-x64-scalar`), like the existing `565`/`f16` goldens. Hashes only, no objects (the renders are large and can be redone from the cached build).

### The runner CPU matters

The hosted runners are a mix of AMD EPYC (Zen 3 `7763`, Zen 4 `9V74`/`9V45`) and Intel Xeon (`8370C`, `8573C`) machines, and a job cannot choose. Intel's `rcpps`/`rsqrtps` estimates differ from AMD's (design `raster-pipeline.md` §4.6), and the baseline (SSE2) and ml3 raster pipeline use them. `run_tiers.sh` therefore fails the job on a non-AMD CPU (`REQUIRE_AMD`, default on; not for `ml4`, whose AVX-512 `rcp14`/`rsqrt14` estimates are specified exactly: an Intel Xeon 8573C and an AMD EPYC 9V45 gave identical hashes for all 7,885 gm, image and lottie results); re-run the failed job (`POST .../actions/runs/<id>/rerun-failed-jobs`) to get another runner. `ml4` needs AVX-512 (Zen 4 or Intel; most AMD runners are Zen 3 and lack it, so the leg may need a few re-runs) and `ml3` needs AVX2/FMA/F16C; a runner without them fails the leg the same way. The oracle host was a Zen 4. Zen 3 and Zen 4 runners gave identical hashes wherever both could run the tier.

### Equivalence with `goldens-m156`

`--src gm --config 8888`, 909 results per tier, against the published RGBA variant tiers (m156, runs on AMD runners):

| Tier | byte-identical | different |
|---|---|---|
| `cpu-x64-sse2-rgba` | 898 | 11 |
| `cpu-x64-sse2-rgba-rt-ml3` | 898 | 11 |
| `cpu-x64-sse2-rgba-rt-ml4` | 898 | 11 |
| `cpu-x64-sse41-rgba` | 898 | 11 |
| `cpu-x64-scalar-rgba` | 896 | 13 |

(98.8 to 99.0 %.) The 11 results that differ on every tier, and the 2 extra on `scalar`, fall into these cause classes:

| Class | GMs | Evidence |
|---|---|---|
| Windows-only GM code | `rasterallocator` (57,956 px) | `gm/rasterhandleallocator.cpp` has separate `SK_BUILD_FOR_MAC`, `SK_BUILD_FOR_WIN` (GDI) and generic implementations; our output equals the golden with R and B swapped. Not reproducible on Linux. |
| libm (glibc vs UCRT), float functions | `parsedpaths` (47 px), `shadow_utils`, `shadow_utils_occl` (and `_gray` on scalar), `gradients_powerless_hue_LCH`, `imagefilter_transformed_image` | skia-rust fails the same GMs against the Windows goldens with the same pixel counts (`parsedpaths`: 47, `notes/gm_arcto_cpp_parsedpaths.md`). The `libm_shim` diagnostic (`oracle/ci/libm_shim.c`: float `sinf`, `cosf`, `tanf`, `atan2f`, `cbrtf`, ... computed in double and rounded, which is how the UCRT implements most of them) makes `parsedpaths`, `shadow_utils*`, `LCH` and `imagefilter_transformed_image` byte-identical to the golden. |
| libm, other functions | `longpathdash` (double `sin`/`cos`, 15 px, 5 with the shim), `gradients_color_space_many_stops` and `gradients_powerless_hue_OKLCH` (`cbrtf`: UCRT's equals neither glibc's nor the double-rounded one; the shim moves `many_stops` from 46 to 181 differing pixels, skia-rust's own number), `blurrect_compare` (1 px), `poster_circle` (scalar tier only, 1 to 3 px) | The same set as the skia-rust libm notes (`notes/gm-gradients.cpp-OKLCH-libm.md`, `notes/gm_dashing_cpp_longpathdash.md`). |
| Unexplained | `scaledemojiperspective_test` (482 px, delta 47) | skia-rust matches the Windows golden here, Linux Skia does not. The GM draws the Test SVG emoji typeface under perspective as drawables. To investigate (expat/SVG or a drawable-path libm call). |
| Intel vs AMD (never on a passing run) | an Intel Xeon 8573C run differed in 32 (baseline) and 44 (ml3) outputs: perspective GMs, `xfermodes*`, `HSL_duck`, `patch_*`, `vertices`; its `ml4` run (`rcp14`) still had only the 11 | The vendor estimate effect. Hence `REQUIRE_AMD`. |

Not a cause: CPU tier dispatch (`oracle_ci.py hash` rejects a result whose metadata says another runtime tier ran, and the tiers behave as the classes in `tiers.toml` say), N32 order (the RGBA variant goldens exist for this; only `rasterallocator` is order-related), fonts (every text GM matches: the portable font manager is pure Skia, with no FreeType or fontconfig), and Skia's bundled zlib, libpng, libjpeg-turbo, expat and ICU, which are the same sources on both hosts.

Verdict: the Linux build reproduces the Windows goldens byte for byte except where libm reaches the pixels (and one Windows-only GM). Both are "real Skia"; where they differ, the host libm decides, which is the same question skia-rust's own libm mismatches ask (the libm port is on hold). Linux-rendered goldens are therefore trustworthy as extra goldens, and a mismatch of ours against a CI-rendered `image` or `lottie` golden that falls into the libm classes above is a libm question, not a port bug.

### Image and lottie results

`--src image` renders 6,789 `8888`, 1,776 `565` and 4,095 `f16` results per tier (28,116 tasks); 106 tasks fail, all on `progressive_kitten_missing_eof.jpg` (a truncated file; partial-decode modes report errors) and produce no result. `--src lottie` renders all 187 files (`8888`, a 1000x1000 filmstrip each). The RGBA `8888` image results are identical on `sse2`, `ml3` and `sse41` (1 result differs on `scalar`); `565`/`f16` differ in 656 results between `sse2` and `ml3`, in none for `sse41` and `scalar`; 36 lottie files differ between `sse2` and `ml3` and 49 between `sse2` and `scalar`. For the Rust side these are per-tier goldens like the `gm` ones.

### Diagnostics

`oracle_ci.py compare` writes a class per mismatch (`n32-byte-order` when swapping R and B of ours equals the golden, `off-by-one`, `pixels`, `size`) with the differing pixel count and maximum channel delta. The `libm_shim` step re-renders only the mismatched GMs with `LD_PRELOAD=oracle/ci/libm_shim.so` and compares again (`equivalence-shim.md`); it is for finding causes, never for goldens. `libm-imports.txt` lists the libm functions the `dm` binary imports.

### Not covered yet

BGRA-order `8888` for image and lottie (the Windows N32; a Linux build with `-DSK_R32_SHIFT=16` would produce it), GPU tiers (phase 2, `docs/design/gpu.md` §3.1), nanobench goldens (`bench.md` B3: the nanobench binary is built and cached, but the `--oracleRawPath` extension of the patch is still to do), the `svg` source and the `pdf`/`svg` sinks, arm64/macOS/Windows runners.

## Usage (on the server)

```
cargo xtask oracle deps                   # git-sync-deps (Dawn, codecs, …), gn, ninja
cargo xtask oracle build x64-sse2         # gn gen + ninja dm, into third_party/skia/out/oracle/<build>
cargo xtask oracle tiers                  # list derived tiers
cargo xtask oracle run cpu-x64-sse2-rt-ml3 --config 8888 f16 --src gm [--match aarect] [--fresh]
cargo xtask oracle compare cpu-x64-sse2-rt-ml3 path/to/our/outputs
cargo xtask oracle extract cpu-x64-sse2-rt-ml3 8888/gm/aarectmodes out.raw
cargo xtask oracle rp-dump cpu-x64-sse2 aarectmodes [--config f16] [--ctx] [--out f.txt]
cargo xtask oracle check-classes          # do the [[class]] tier groups in tiers.toml still hold?
```

`rp-dump` runs DM single-threaded for one GM with `SKIA_ORACLE_RP_DUMP=<file>` (added by the oracle patch to `SkRasterPipeline::buildPipeline`, so it sees every `run()` and `compile()`). Each pipeline is one line, `<result id> <highp|lowp> <op> <op> ...` with Skia's op names, followed by `# <index> <op> <values>` lines for stages whose context matters to exactness (`uniform_color`, `set_rgb`, `matrix_*`, `parametric`, `gamma_`, 2-stop gradients, gradient stop counts); `--ctx` prints those too. The raw file is kept in `target/oracle-rp-dump/<tier>/<config>/<gm>.txt`. DM sets the result id per task, so `SKIA_ORACLE_RP_DUMP` also works with a plain `cargo xtask oracle run` (it appends; delete the file first).

`cargo xtask oracle rp-diff [--tier ml3,ml4] [--case-glob 'srcover/*'] [--update] [--replay]` is the per-stage raster pipeline oracle (`rp-diff/`, design `docs/design/raster-pipeline.md` §4.2): a small C++ driver, built against the `x64-sse2` build's static libraries with the oracle's flags and one `SkRasterPipeline_opts.h` instantiation per x86 code path, runs the cases of `rp-diff/src/cases.rs` through Skia's own `SkRasterPipeline`; xtask replays them through skia-rust and reports the first differing bytes. `--update` stores Skia's results as hashes in `rp-diff/expected/<tier>.txt` (committed), which `cargo test` replays on every host without C++. How to add cases: `docs/PORTING.md` §12.

`scan-aaa/build.ps1` is the scan conversion oracle (task C3): a C++ harness, built against the `x64-sse2` libraries like `rp-builder`, runs the cases of `crates/skia-rust-raster/src/scan_aaa_tests/cases.txt` through Skia's `SkScan` (`AntiFillPath`, `AntiFillRect`, `AntiFillXRect`, `AntiFrameRect`) with a blitter that prints every call, and writes `scan_aaa_tests/skia_dump.txt`, which `cargo test` compares with skia-rust's calls (`skia_rust_raster::blitter_dump::DumpBlitter::oracle_text`). Run it after editing the cases: `./build.ps1 -Skia <path to third_party/skia>`.

`rp-builder/build-d4.ps1` is the legacy blitter oracle (task D4): `d4_blitters.cpp` runs Skia's `SkARGB32_*_Blitter`, `SkA8_*_Blitter`, `SkBlitter::Choose`/`ChooseSprite` blitters and `SkBlitRow` procs on deterministic inputs (every `blit*` call, strided devices, all mask formats) on the `x64-sse2` build at the baseline CPU tier and writes one hash per step to `crates/skia-rust-raster/src/skia_d4_dump.txt`, (and `_ml3`/`_ml4` runs under those CPU caps, for the pipeline sprite lines), which `legacy_blitters_tests.rs` compares with skia-rust's blitters on every x86 tier. Pass `full` to the executable for the pixels. Run it after changing the generators on either side: `./build-d4.ps1 -Skia <path to third_party/skia>`.

`draw/build.ps1` is the CPU draw layer oracle (task D5): `draw.cpp` interprets the case script `crates/skia-rust-raster/src/draw_tests/cases.txt` (generated by `draw/gen_cases.py`) with a real `SkBitmapDevice` and `skcpu::Draw`, hashing the device, and writes one dump per x86 code path (`skia_dump{,_sse41,_ml3,_ml4,_scalar}.txt`, from the `x64-sse2`, `x64-sse41` and `x64-scalar` builds and the `SKIA_ORACLE_CPU_CAP` caps), which `draw_tests.rs` compares with `BitmapDevice` on every tier. Debugging: `DRAW_ONLY=<case>` runs one case, `DRAW_PIXELS=1` prints the hashed bytes to stderr (the Rust test honours both).

`check-classes` reads the `[[class]]` entries of `tiers.toml` (the measured equivalence classes below), loads each tier's `hashes.json`, and fails if tiers inside a class differ on any result (split) or two classes become identical (merge). Run it after every golden publish.

Result ids are `<config>/<src>/[<options>/]<name>`, e.g. `8888/gm/aarectmodes`. Goldens live in `goldens/<skia-commit>/` (git-ignored):

- `objects/<sha[..2]>/<sha>.zst`: each distinct output stored once, zstd-compressed. Most outputs are identical across tiers, so the full matrix stays small.
- `<tier>/hashes.json`: result id → SHA-256 of the raw bytes. This is all `compare` needs.
- `<tier>/meta.json`: result id → size, color type, alpha type, serialized color space.
- `<tier>/toolchain.txt`: compiler version and GN args.

`cargo xtask oracle publish` uploads the hashes, metadata and objects to the `goldens-<mNNN>` GitHub release and records them in `inventory/goldens.lock`.

`run` merges into a tier's existing results, so a tier can be filled in several `--match` runs; `--fresh` starts over. If some sources fail, everything that rendered is still stored and the run then reports DM's failure. Hash files and objects are published as described in PLAN §5.3.

Prerequisites on Windows: Visual Studio 2022 Build Tools (MSVC + Windows SDK), Python 3, and LLVM (`clang-cl`) at `C:\Program Files\LLVM` or wherever `SKIA_ORACLE_CLANG_WIN` points.

## Open problems

- **Fontations in GN builds needs Bazel.** Skia's GN build compiles its Rust (Fontations, via `cxx`) by invoking Bazel and linking the resulting static libraries. Whether that works on Windows is untested. Fallbacks: build the CPU text oracle on Linux (WSL2), or add a GN-native Rust build of the Fontations bridge.
- **lavapipe isn't set up yet.** It needs a Linux build of the oracle under WSL2. D3D12/WARP (gating) and the RTX 4070 SUPER on D3D12 and Vulkan (report only) work. Cross-host determinism of WARP (the same goldens on GitHub's Windows runners) is still unmeasured.
- **Graphite caps aren't restricted to wgpu's feature set yet** (PLAN §5.1). `skiatest::graphite::AddPreferredFeatures` requests every optional Dawn feature the adapter has; the oracle needs a filter limiting it to features wgpu also exposes.
- **arm64 and wasm tiers** need their own hosts and toolchains (PLAN §5.1).

## Findings

- **2026-10-09, skcms ignores the CPU cap:** skcms does its own `cpuid` dispatch (`modules/skcms/skcms.cc` `cpu_type()`), not `SkCpu`, so `SKIA_ORACLE_CPU_CAP` and the compile-time baselines do not reach it: on the Zen 4 oracle host **every x64 tier ran the SKX kernel**. HSW/SKX convert half floats with F16C (`vcvtps2ph`, round to nearest even, denormals kept) where the baseline kernel truncates, so e.g. the RGBA_F16 decode of `color_wheel.png` (`EncodeSRGBGM`) differs from the baseline by a half step. `skia-rust-skcms` models this (`src/cpu.rs`): while a tier is forced it behaves as SKX on any host; otherwise it decodes the real host. The skcms-diff harness is built `SKCMS_PORTABLE` (baseline), so its Rust twin must call `disable_runtime_cpu_detection()`.
- **2026-10-06, scalar proxy build `x64-scalar` (`-DSKRP_CPU_SCALAR`, tier `cpu-x64-scalar`):** **811 of 2,727 outputs differ from `cpu-x64-sse2`** (565: 490, 8888: 227, f16: 94; 1,582 differ from the ml3 class), so the scalar raster pipeline is a fifth behaviour and gets its own goldens and, in skia-rust, its own code path (`Scalar`). Rendering the full suite took one DM run with no failures. `check-classes` now covers 5 classes (the four x64 behaviours and `scalar-proxy`) and passes. Build note: a full oracle build must run from a short path (`C:
6` junction); Dawn/partition_alloc includes exceed MAX_PATH under `.claude/worktrees/...`.
- **2026-10-06, m156, build `x64-sse2`:** runtime tiers produce different pixels. On `--match ^gradients` (35 GMs × `8888`, `f16`), 62 of 70 outputs differ between `cpu-x64-sse2` and `cpu-x64-sse2-rt-ml3`, and the perspective-gradient GMs differ again between `ml3` and `ml4`. `ssse3` matched `sse2` on that slice. Simple stroke/AA-rect GMs were identical across all four runtime tiers. Per-tier goldens are required, as the plan assumed.
- **2026-10-06, build flags:** raising a build's baseline with `/arch:` in `extra_cflags` silently lowers Skia's ml3/ml4 kernels, because GN appends `extra_cflags` after each target's own `/arch:AVX2` / `/arch:AVX512`. A 19-tier comparison caught it: `x64-avx-rt-ml3` came out identical to the SSE2 baseline. Baselines now use additive `/clang:-m…` flags only.
- **2026-10-06, Dawn on Windows:** Skia builds Dawn with CMake. Dawn enables a C++20-modules target when `clang-scan-deps` exists, but CMake can't scan modules for clang-cl, so the oracle patch passes `-DDAWN_SUPPORTS_CXX_MODULES=OFF` in `third_party/dawn/build_dawn.py`. xtask also puts VS's bundled CMake and Skia's ninja on `PATH`, which Dawn's build script needs.
- **2026-10-06, all 19 x64 tiers** (88 outputs: gradients, AA rects, strokes; `8888` + `f16`) fall into 5 distinct behaviours: {sse2…avx baselines, ssse3 runtime}, {any baseline + ml3 runtime}, {any baseline + ml4 runtime}, {v3 compile-time baseline}, {v3 + ml4 runtime, v4}. A compile-time AVX2 build differs from runtime ml3 dispatch.
- **2026-10-06, GPU:** WARP (Microsoft Basic Render Driver, D3D12 10.0.26100.9278) is deterministic across 3 runs on this host, with 1 and 4 DM threads. WARP, RTX D3D12, RTX Vulkan and the CPU tiers share 0 of 44 hashes, so GPU goldens are per environment.
- **2026-10-06, FMA contraction:** Skia builds with `-ffp-contract=off` for clang on every platform except Windows, where its `/fp:precise` makes clang-cl use `-ffp-contract=on` and fuse `a*b+c` into FMA wherever the target has FMA (ml3/ml4 kernels, v3/v4 builds). Checked on a one-line test: 1 `vfmadd` with `/fp:precise`, 0 with `/clang:-ffp-contract=off`. All oracle builds now pass `-ffp-contract=off` (`[gn] extra_cflags`), so goldens match Skia as built on Linux/macOS and by Chrome. skia-rust must likewise use `mul_add` only where Skia calls an explicit FMA.
- **2026-10-06, first full tier:** `cpu-x64-sse2` rendered 2,727 results (all GMs × `8888`/`f16`/`565`) in 208 s with no DM failures; `cpu-x64-sse2-rt-ssse3` added 0 new objects, i.e. it is byte-identical to `sse2` on the whole suite. (Pre-fp-contract fix; superseded by the full run below.)
- **2026-10-06, full suite, fp-contract off:** every GM × `8888`/`f16`/`565` (2,727 results per CPU tier) on all 19 x64 tiers collapses into **4 distinct behaviours**: (1) `sse2`/`ssse3` baselines and the `ssse3` runtime tier; (2) `sse41`/`sse42`/`avx` baselines (52 outputs differ from 1); (3) any baseline with `ml3` runtime dispatch, and the compile-time `v3` build (1,020 differ from 1); (4) any baseline with `ml4` dispatch, `v3`+`ml4`, and `v4` (1,020 differ from 1; differ from 3 too). skia-rust therefore needs four x64 code paths. All tiers keep their own hash files.
- **2026-10-06, GPU:** 910 Graphite results per GPU tier. WARP, RTX D3D12 and RTX Vulkan all log the same 227 "Key context creation failed ... draw dropped" and 123 "Path effect failed to apply" warnings, so these are Skia/Graphite behaviours (part of what we must match), not environment problems.
- **2026-10-06, published** as release `goldens-m156` (`inventory/goldens.lock`): 22 tiers, 451 MB of unique objects.
- **2026-10-06, RGBA variants (`SK_R32_SHIFT=0`):** 5 tiers x 909 `8888` results, every one `RGBA_8888`. Swapping R and B of each BGRA counterpart (`cpu-x64-sse2`, `-rt-ml3`, `-rt-ml4`, `cpu-x64-sse41`, `cpu-x64-scalar`) reproduces **901 of 909** outputs byte for byte, on every tier. The same 8 differ on every tier: `rasterallocator` (57,956 of 180,000 pixels, channel delta up to 255: the GM draws with a fixed byte order), `bitmapfilters` (159 pixels, delta up to 221), `pathopsinverse` (12,100 pixels, delta up to 13) and five `pictureshader*` GMs (38 to 1,751 pixels, delta 1: the picture is rasterized to an N32 tile whose rounding depends on the channel order). So an RGBA golden is not always the swizzled BGRA one, and the variant goldens are needed. `check-classes` now covers 10 classes and passes.

- **2026-10-07, rp-diff (B5/B6a-c):** 2,986 more per-stage cases (geometry/tiling and `SkSL` stages; 6,410 in all with the memory cases) match Skia on all five x86 code paths, natively and through the models, after two fixes: `Scalar` evaluated `smoothstep` and `refract` in `float` where C++ promotes their `double` literals, and `Ml4`'s `cast_to_uint_from_*` used the signed conversion instead of `vcvttps2udq`. 258 cases are not compared on `Scalar` (float → int conversions out of range, `±0` in `fminf`: wasm's libc vs the x64 proxy, design R5). The trace ops (B6d) have no cases (host hook).
- **2026-10-07, rp-diff (A5):** 516 per-stage cases of the A3 stages match Skia byte for byte on all five x86 code paths (`scalar`, `sse2`, `sse41`, `ml3`, `ml4`), natively and through the models. Only where two NaN inputs meet in a fused `mad` did ml3/ml4 differ (Skia's clang and rustc pick different `vfmadd…` forms, so a different NaN wins); that is codegen, not Skia's source, and the cases now keep NaNs one per lane (design §4.2, "As implemented in A5").

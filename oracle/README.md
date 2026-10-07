# The oracle

Real Skia, built from the pinned checkout (`inventory/skia-pin.toml`), is the reference every skia-rust output is compared against. It is test tooling only and never a dependency of a published crate. See `docs/PLAN.md` §5.

## How it works

The oracle **is Skia's own test runner, DM**, with one small addition. DM already knows every GM, image, Skottie and SVG source and every color config, and it renders them exactly the way Skia's bots do. We don't reimplement any of that. We add:

| File | What it does |
|---|---|
| `dm/OracleDump.{h,cpp}` | Adds `--oracleRawPath <dir>` to DM. For every result it writes the exact bytes: `.raw` for pixels (rows tightly packed, `minRowBytes` each) or `.bin` for encoded output (PDF, SVG, SKP), plus a `.json` with size, color type, alpha type, serialized color space and the CPU tier that actually ran. |
| `patches/skia-oracle.patch` | Hooks `OracleDump` into DM's result path and adds it to the `dm` GN target. Adds `SKIA_ORACLE_CPU_CAP` to `SkCpu`, which hides runtime CPU features above a level so one binary can produce every runtime tier. Adds `SKIA_ORACLE_DAWN_ADAPTER` to Graphite's Dawn test context, which picks the adapter by name (WARP, lavapipe, a real GPU) and fails rather than falling back. |
| `tiers.toml` | The GN builds (one per compile-time x86 baseline, plus Graphite+Dawn) from which xtask derives CPU tiers. |

`cargo xtask oracle patch` copies `dm/` into Skia and applies the patch (idempotent). On a pin bump, if the patch no longer applies, regenerate it against the new milestone; it is small on purpose.

## CPU tiers

Skia picks x86 code in two places: the compile-time baseline (`SK_CPU_X64_LEVEL`) and runtime `SkOpts` dispatch (ssse3, ml3 = x86-64-v3, ml4 = x86-64-v4). A tier is one (build, runtime level) pair, and xtask derives every pair a shipped Skia can run. `cargo xtask oracle tiers` lists them.

Each run checks that every result actually ran at the requested level. If the host CPU lacks a feature, for example AVX-512 for `ml4`, the run is rejected rather than silently producing a lower tier's output. The Ryzen 7 7800X3D (Zen 4) runs every x86 tier natively.

All builds use clang-cl, the compiler Chrome ships Skia with. Every run records the compiler version and GN args in `toolchain.txt` next to the goldens, because a different compiler can change floating-point codegen (FMA contraction) and therefore pixels.

## Fonts

Runs use `--nativeFonts false`, i.e. Skia's portable test font manager, which is pure Skia and identical on every platform. GMs that load real font files go through whatever typeface factory the build has. **Text goldens are not trustworthy until the Fontations build works** (`skia_use_fontations=true`; see "Open problems").

## Usage (on the server)

```
cargo xtask oracle deps                   # git-sync-deps (Dawn, codecs, …), gn, ninja
cargo xtask oracle build x64-sse2         # gn gen + ninja dm, into third_party/skia/out/oracle/<build>
cargo xtask oracle tiers                  # list derived tiers
cargo xtask oracle run cpu-x64-sse2-rt-ml3 --config 8888 f16 --src gm [--match aarect] [--fresh]
cargo xtask oracle compare cpu-x64-sse2-rt-ml3 path/to/our/outputs
cargo xtask oracle extract cpu-x64-sse2-rt-ml3 8888/gm/aarectmodes out.raw
```

Result ids are `<config>/<src>/[<options>/]<name>`, e.g. `8888/gm/aarectmodes`. Goldens live in `goldens/<skia-commit>/` (git-ignored):

- `objects/<sha[..2]>/<sha>.zst`: each distinct output stored once, zstd-compressed. Most outputs are identical across tiers, so the full matrix stays small.
- `<tier>/hashes.json`: result id → SHA-256 of the raw bytes. This is all `compare` needs.
- `<tier>/meta.json`: result id → size, color type, alpha type, serialized color space.
- `<tier>/toolchain.txt`: compiler version and GN args.

`run` merges into a tier's existing results, so a tier can be filled in several `--match` runs; `--fresh` starts over. If some sources fail, everything that rendered is still stored and the run then reports DM's failure. Hash files and objects are published as described in PLAN §5.3.

Prerequisites on Windows: Visual Studio 2022 Build Tools (MSVC + Windows SDK), Python 3, and LLVM (`clang-cl`) at `C:\Program Files\LLVM` or wherever `SKIA_ORACLE_CLANG_WIN` points.

## Open problems

- **Fontations in GN builds needs Bazel.** Skia's GN build compiles its Rust (Fontations, via `cxx`) by invoking Bazel and linking the resulting static libraries. Whether that works on Windows is untested. Fallbacks: build the CPU text oracle on Linux (WSL2), or add a GN-native Rust build of the Fontations bridge.
- **lavapipe isn't set up yet.** It needs a Linux build of the oracle under WSL2. D3D12/WARP (gating) and the RTX 4070 SUPER on D3D12 and Vulkan (report only) work. Cross-host determinism of WARP (the same goldens on GitHub's Windows runners) is still unmeasured.
- **Graphite caps aren't restricted to wgpu's feature set yet** (PLAN §5.1). `skiatest::graphite::AddPreferredFeatures` requests every optional Dawn feature the adapter has; the oracle needs a filter limiting it to features wgpu also exposes.
- **arm64 and wasm tiers** need their own hosts and toolchains (PLAN §5.1).

## Findings

- **2026-10-06, m156, build `x64-sse2`:** runtime tiers produce different pixels. On `--match ^gradients` (35 GMs × `8888`, `f16`), 62 of 70 outputs differ between `cpu-x64-sse2` and `cpu-x64-sse2-rt-ml3`, and the perspective-gradient GMs differ again between `ml3` and `ml4`. `ssse3` matched `sse2` on that slice. Simple stroke/AA-rect GMs were identical across all four runtime tiers. Per-tier goldens are required, as the plan assumed.
- **2026-10-06, build flags:** raising a build's baseline with `/arch:` in `extra_cflags` silently lowers Skia's ml3/ml4 kernels, because GN appends `extra_cflags` after each target's own `/arch:AVX2` / `/arch:AVX512`. A 19-tier comparison caught it: `x64-avx-rt-ml3` came out identical to the SSE2 baseline. Baselines now use additive `/clang:-m…` flags only.
- **2026-10-06, Dawn on Windows:** Skia builds Dawn with CMake. Dawn enables a C++20-modules target when `clang-scan-deps` exists, but CMake can't scan modules for clang-cl, so the oracle patch passes `-DDAWN_SUPPORTS_CXX_MODULES=OFF` in `third_party/dawn/build_dawn.py`. xtask also puts VS's bundled CMake and Skia's ninja on `PATH`, which Dawn's build script needs.
- **2026-10-06, all 19 x64 tiers** (88 outputs: gradients, AA rects, strokes; `8888` + `f16`) fall into 5 distinct behaviours: {sse2…avx baselines, ssse3 runtime}, {any baseline + ml3 runtime}, {any baseline + ml4 runtime}, {v3 compile-time baseline}, {v3 + ml4 runtime, v4}. A compile-time AVX2 build differs from runtime ml3 dispatch.
- **2026-10-06, GPU:** WARP (Microsoft Basic Render Driver, D3D12 10.0.26100.9278) is deterministic across 3 runs on this host, with 1 and 4 DM threads. WARP, RTX D3D12, RTX Vulkan and the CPU tiers share 0 of 44 hashes, so GPU goldens are per environment.

# Session 001: planning and Phase 0 scaffolding

Curated transcript of the planning session (conversation and decisions only; tool output omitted). Early messages contain ideas that were later revised; `docs/PLAN.md` is authoritative.

## User

I have been getting into Rust lately, and I have found that the major option for 2D graphics are Skia bindings or a tiny Skia subset. There have been a few attempts to rewrite Skia itself into Rust for the best of both worlds, but nothing concrete or feature-complete. I found one vibecoded attempt in particular (https://github.com/quinnjr/skia-rs) that while nowhere near what the readme promises, is not nothing. That made me think, "what if I vibecoded my own Skia re-implementation but with more comprenensive testing?"

## Claude

It's a reasonable project. The testing idea matters most, because with real Skia available as an oracle you can check every claim against it instead of relying on what a README says. Your directory is empty, so here's how I'd set it up before writing any code.

**Testing (the part skia-rs skipped)**
- **Compare against real Skia.** Add `skia-safe` as a dev-dependency only. Every test draws the same scene with both your crate and Skia, then compares the pixels: exact match for solid fills, a small per-channel tolerance for anti-aliased edges.
- **Port Skia's GM tests.** Skia ships hundreds of "GM" scenes (`gm/*.cpp`). Porting them one at a time gives you a measurable progress number ("142/600 GMs pass") instead of a feature list.
- **Property tests and fuzzing.** Use `proptest` with random paths and transforms, checked against Skia. Use `cargo-fuzz` on path ops and stroking, which is where Skia's hard edge cases live.
- **Golden images in CI.** CI can't easily build Skia, so store reference PNGs from Skia and diff against them in CI. Run the comparison against Skia itself locally.

**Scope**
Skia is millions of lines, so pick a core and make it match Skia exactly before going wider:
1. Geometry: `Point`, `Rect`, `Matrix`, `Path` with bounds and iteration
2. Raster: scan conversion with Skia's anti-aliasing (supersampling, as in `SkScan_AntiPath`), fills and fill rules
3. `Paint`: solid colors, all blend modes, opacity
4. Stroker: joins, caps, dashes
5. Clipping, `save`/`restore`, the full `Canvas` API
6. Shaders: linear, radial and conic gradients, image shaders, sampling
7. Later: path ops, image filters, text (pair it with `swash` or `skrifa` rather than rewriting FreeType), PDF and SVG output, a GPU backend

tiny-skia already covers roughly steps 1–6. To be worth doing, yours needs a different goal: API compatibility with Skia, a high pass rate against Skia itself, or a roadmap past what tiny-skia does (path ops, filters, text).

**Process**
Write the comparison harness before any rendering code. Each feature then lands only alongside the tests that check it against Skia, which keeps a vibecoded codebase honest.

One practical catch: `skia-safe` downloads prebuilt binaries on Windows, which usually works but sometimes needs LLVM or clang installed.

If you want, I can scaffold it now: a Cargo workspace with the core crate, a test-harness crate that compares against `skia-safe`, the first geometry types, and one passing comparison test.

## User

My goals would be:

1. Actually hit the 100% compatibility promise through Skia's actual tests
2. Match the Skia API but with idiomatic Rust
3. Support wgpu rendering which by extension supports just about every graphics API out there

## Claude

Those goals work. Each one changes something about the plan.

**1. Hitting 100% on Skia's own tests**

You need to define the denominator before you start counting:
- **Unit tests** (`tests/*.cpp`, `DEF_TEST`) are pass/fail assertions. Those port cleanly and count cleanly.
- **GMs** (`gm/*.cpp`, `DEF_GM`) have no expected images in the repo. Skia keeps its reference images on its own server (Gold), separately for each backend. So "pass" has to mean "matches real Skia's CPU output at a pinned version, within a stated tolerance". Generating those reference images yourself with `skia-safe` gives you that.
- **Pin one Skia milestone**, whichever one `skia-safe` currently wraps, so both the oracle and the test sources come from the same commit. Upgrade on purpose, not by drift.
- **Build an inventory tool first.** It would parse every `DEF_TEST`/`DEF_GM` at the pinned commit into a manifest where each test is marked ported, passing, failing, or excluded with a reason. A small share of tests check Skia internals (`SkTDArray`, `SkSL` details) and have no Rust equivalent. Mark those as excluded and say so publicly, or the 100% claim won't hold up.
- **Porting is mostly mechanical**, which suits an LLM. Keeping the test code reading close to the C++ makes the ports reviewable.

**2. The Skia API in idiomatic Rust**

This pulls against goal 1. The closer your API is to Skia's, the cheaper it is to port thousands of tests. So keep Skia's concepts and names (`Canvas`, `Paint`, `Path`, `Shader`, `draw_rect`) and make the Rust changes systematically:
- Use `Arc` or plain values instead of `sk_sp`.
- Return `Option`/`Result` where Skia returns a null pointer.
- Use enums and bitflags in place of integer flags.
- Use builders where Skia has many optional parameters.
- Have `Canvas` borrow its surface.

Writing these rules down as a mapping document keeps them consistent across all the vibecoded work.

**3. wgpu**

Skia's newer GPU backend, Graphite, already treats WebGPU as a main target through Dawn, Google's WebGPU implementation. So porting Graphite with wgpu in Dawn's place fits much better than porting the older Ganesh backend. Two things to know:
- **Shader language.** Skia's custom-effect API (`SkRuntimeEffect`) depends on SkSL, Skia's own shader language. You'd need SkSL → WGSL, probably by parsing SkSL into naga's intermediate representation. That's a large sub-project, so plan it as one.
- **GPU output never matches CPU output pixel for pixel.** GPU GMs would be compared against Skia's own GPU output with looser tolerances. CI can run wgpu without a graphics card using a software adapter: WARP on Windows, lavapipe on Linux.

**Order of work:** inventory and comparison harness → geometry → CPU rendering, until the GMs mostly pass → wgpu on Graphite's design. CPU comes first because it's the reference everything else is checked against.

I can scaffold this now: a Cargo workspace with the core crate, a `skia-oracle` crate wrapping `skia-safe`, a `tests-inventory` tool that pulls the pinned Skia commit and writes the manifest, and the first geometry types with comparison tests. Want me to go ahead?

## User

The underlying code should also be in safe, idiomatic Rust

## Claude

Agreed. I'd enforce it from day one at the workspace level so nothing can quietly drift in:

```toml
[workspace.lints.rust]
unsafe_code = "forbid"
```

What that means in practice:

- **Speed without `unsafe`.** Skia's CPU renderer gets its speed from hand-written vector (SIMD) code. Safe ways to get close:
  - Write the inner loops over fixed-size arrays such as `[f32; 8]` so the compiler vectorizes them itself. This is the main approach.
  - Optionally add per-CPU fast paths with `#[target_feature]`. Since Rust 1.86 those can be safe functions, and most `std::arch` vector intrinsics are safe to call inside them.
  - Avoid `std::simd`, which still needs nightly Rust.
- **Pointer-heavy structures.** Skia's caches and linked lists become index-based arenas or `Vec`s, and shared objects use `Arc` instead of reference counting. Usually no slower, and a better fit for the borrow checker.
- **Dependencies are a separate question.** wgpu, naga and font crates use `unsafe` inside behind safe APIs. `forbid` covers your code only, which is the reasonable line. `skia-safe` is a test-only dependency and never ships in your library.
- **Unchecked bounds.** Avoid hand-rolled unchecked indexing. Write loops so the compiler can prove the indices are in range, and let benchmarks find any real hotspots.

I'd also turn on `clippy::pedantic` (warn level) and run `cargo deny` to check dependency licenses and security advisories.

The scaffold plan is the same as before, now with these lints and a `benches/` crate added so speed can be compared against Skia. Ready to build it when you say go.

## User

Adjustments:

1. unsafe will be necessary for SIMD, but it should not be abused to get away with sloppy code like in C++
2. GPU output should match Skia GPU output pixel for pixel since that is where the arithmetic should be the exact same, not CPU output loosely.
3. clippy::pedantic agreed

## Claude

All three are in the plan now. Two of them have details worth settling up front.

**1. `unsafe` only for SIMD, under strict rules**
- Workspace-wide `unsafe_code = "deny"`. It's lifted in one crate only, `skia-simd`, and nowhere else.
- In that crate, the lints `clippy::undocumented_unsafe_blocks`, `unsafe_op_in_unsafe_fn` and `clippy::multiple_unsafe_ops_per_block` are turned on. Every `unsafe` block has to be tiny and come with a `// SAFETY:` comment explaining why it's sound.
- Every SIMD routine also has a plain scalar version. Tests require the two to produce bit-identical results, and Miri (Rust's undefined-behavior checker) runs over the crate.
- With safe `#[target_feature]` functions (Rust 1.86+), `unsafe` mostly shrinks to the call made after runtime CPU-feature detection, plus raw loads and stores.

**2. GPU output identical to Skia's GPU output**

Agreed, and CPU output should be exact too. I was wrong earlier to suggest a tolerance there: Skia's CPU renderer is deterministic. Exact matching needs everything except your code held constant:
- **Same machine setup.** Fix the GPU API, driver and adapter. Software drivers (lavapipe for Vulkan, WARP for D3D12) are deterministic and run in CI, so they make good reference environments. Results on real hardware can be reported but shouldn't gate tests.
- **Same shaders, as close as possible.** Skia's Graphite compiles WGSL through Dawn's compiler (Tint), while wgpu uses naga. Each produces different backend code, and drivers may fuse operations (FMA) differently. If your WGSL is a faithful translation of Graphite's, outputs should match. Any mismatch then points to a translation bug rather than tolerance noise.
- **CPU feature level.** Skia's CPU renderer picks different SIMD code paths at runtime (SSE2, AVX2, and low- versus high-precision pipelines), and they don't all give identical pixels. The oracle needs one pinned feature level, and your implementation has to match the behavior of that tier.
- **The GPU oracle.** I'm not sure `skia-safe` exposes Graphite; it may only wrap the older Ganesh backend. If it doesn't, the oracle needs Skia built from source with Graphite and Dawn, plus a small C++ program that renders each GM to PNG. That's test tooling only and never ships.

**3. `clippy::pedantic`:** on, at warn level.

I'll check `skia-safe`'s Graphite support first, then scaffold the workspace: core, `skia-simd`, oracle, inventory and benches crates, with the lints configured. Should I start?

## User

Let's first create a comprehensive plan for how the project will be created, tested, and maintained. AskUserQuestion for all decisions

## Claude

`skia-safe` 0.99 currently wraps Skia milestone m155. It has a `graphite` feature but no Dawn backend, which bears on the GPU-oracle decision. Here's the first set of decisions.

## Claude asked

- Which license? Code ported from Skia is a derivative of Skia's BSD-3-Clause license, so Skia's copyright notice must be kept either way.
  - BSD-3-Clause (Recommended): Same license as Skia. The simplest legal story for line-by-line ports and translated tests.
  - MIT OR Apache-2.0: The Rust ecosystem's usual dual license, plus a NOTICE file carrying Skia's BSD-3 attribution for ported code.
  - Apache-2.0 only: Includes an explicit patent grant. Skia attribution still required.
- Edition and minimum supported Rust version (MSRV) policy?
  - 2024, latest stable (Recommended): Edition 2024, MSRV = current stable at each release. Gets the newest safe target_feature and SIMD improvements.
  - 2024, stable minus 2: Supports the last 3 stable releases. Friendlier to downstream users, slightly slower to adopt new features.
  - 2024, fixed MSRV: MSRV bumped only in minor releases and documented.
- Crate organization?
  - Workspace + facade (Recommended): Internal crates (core, path, raster, gpu, text, codec, simd…) re-exported through one facade crate with feature flags. Faster builds, enforced layering, one dependency for users.
  - Single crate + features: One crate with modules behind cargo features. Simpler publishing, but slow incremental builds and no enforced layering.
  - Mirror Skia's module dirs: One crate per Skia top-level module (core, effects, pathops, gpu/graphite, …). Easiest side-by-side comparison with the C++ tree.
- Project/crate name? (I'll check crates.io availability before using it.)
  - skia-rust: Matches your repo folder.
  - Pick a distinct name: A non-'skia' name that avoids confusion with skia-safe/skia-rs. Tell me via Other, or I'll propose some.
  - Decide later: Use a placeholder for now; renaming before the first publish is cheap.

## User answered

Your questions have been answered: "Which license? Code ported from Skia is a derivative of Skia's BSD-3-Clause license, so Skia's copyright notice must be kept either way."="BSD-3-Clause (Recommended)", "Edition and minimum supported Rust version (MSRV) policy?"="2024, latest stable (Recommended)", "Crate organization?"="Workspace + facade (Recommended)", "Project/crate name? (I'll check crates.io availability before using it.)"="skia-rust"

## Claude asked

- Which Skia version is the oracle pinned to?
  - Track skia-safe's milestone (Recommended): Currently m155. The oracle comes prebuilt for CPU and Ganesh, and test sources come from the same commit. Bump when skia-safe bumps.
  - Pin a milestone, custom build: Build Skia yourself at a chosen chrome/mNNN branch. Full control (Dawn, debug flags) at the cost of a heavy build.
  - Follow Skia main: Always current, but the target moves constantly and test counts churn.
- GPU oracle for exact-match tests? skia-safe has Graphite but no Dawn backend.
  - Custom Graphite+Dawn build (Recommended): Build Skia + Dawn from source with a small C++ GM-dump tool. Same WGSL that Skia emits, so the shader arithmetic is closest to what wgpu will run.
  - skia-safe Graphite+Vulkan: No custom build. But Skia compiles SkSL straight to SPIR-V while wgpu goes WGSL→naga→SPIR-V, so exact matches are harder to diagnose.
  - Both: Dawn as the exact-match oracle; Vulkan as an extra cross-check.
- Reference GPU environment(s) whose output must match exactly (and gate CI)?
  - Vulkan / lavapipe (Linux): Deterministic software Vulkan. Runs on free Linux CI runners.
  - D3D12 / WARP (Windows): Deterministic software D3D12. Matches your dev machine.
  - Real hardware (report only): Run on your GPU and report diffs without gating, to catch driver-specific issues.
- Skia's CPU renderer picks code paths by CPU feature level, and they differ in output. Which tier must match exactly?
  - Pin one tier, force it in oracle (Recommended): Force the oracle to a single tier (e.g. its HSW/AVX2 path) and match it exactly. Other tiers in our code must produce the same pixels.
  - Match every Skia tier: Reproduce SSE2, AVX2 and others with tier-specific behavior. Most faithful, by far the most work.
  - Portable/scalar path only: Match Skia's baseline (SKVM-off, scalar/SSE2) output. Simplest arithmetic to match, but you'd never verify the fast paths.

## User answered

The user answered: "Which Skia version is the oracle pinned to?"="[User dismissed — do not proceed, wait for next instruction]", "GPU oracle for exact-match tests? skia-safe has Graphite but no Dawn backend."="[User dismissed — do not proceed, wait for next instruction]", "Reference GPU environment(s) whose output must match exactly (and gate CI)?"="[User dismissed — do not proceed, wait for next instruction]", "Skia's CPU renderer picks code paths by CPU feature level, and they differ in output. Which tier must match exactly?"="[User dismissed — do not proceed, wait for next instruction]"

## Claude

You closed the second set of questions without answering, so I've stopped there. What's settled so far:

- **License:** BSD-3-Clause.
- **Rust version:** edition 2024, targeting the latest stable release.
- **Layout:** a workspace of internal crates re-exported through one facade crate.
- **Name:** `skia-rust`. I haven't checked crates.io availability yet.

Still open:

- **Skia version:** which release to pin both the oracle and the test sources to.
- **GPU oracle:** `skia-safe` has Graphite but no Dawn backend, so matching Skia's WGSL output would mean building Skia with Dawn from source.
- **GPU reference environment:** which drivers must match exactly and gate CI.
- **CPU feature level:** which of Skia's code paths must match exactly.

Tell me when you want to continue, or what to change about those questions.

## User

skia-rust is available. i checked

## Claude

Thanks, `skia-rust` it is. Picking up the remaining decisions:

## Claude asked

- Which Skia version is the oracle pinned to?
  - Track skia-safe's milestone (Recommended): Currently m155. The oracle comes prebuilt for CPU and Ganesh, and test sources come from the same commit. Bump when skia-safe bumps.
  - Pin a milestone, custom build: Build Skia yourself at a chosen chrome/mNNN branch. Full control (Dawn, debug flags) at the cost of a heavy build.
  - Follow Skia main: Always current, but the target moves constantly and test counts churn.
- GPU oracle for exact-match tests? skia-safe has Graphite but no Dawn backend.
  - Custom Graphite+Dawn build (Recommended): Build Skia + Dawn from source with a small C++ GM-dump tool. Same WGSL that Skia emits, so the shader arithmetic is closest to what wgpu will run.
  - skia-safe Graphite+Vulkan: No custom build. But Skia compiles SkSL straight to SPIR-V while wgpu goes WGSL→naga→SPIR-V, so exact matches are harder to diagnose.
  - Both: Dawn as the exact-match oracle; Vulkan as an extra cross-check.
- Reference GPU environment(s) whose output must match exactly (and gate CI)?
  - Vulkan / lavapipe (Linux): Deterministic software Vulkan. Runs on free Linux CI runners.
  - D3D12 / WARP (Windows): Deterministic software D3D12. Matches your dev machine.
  - Real hardware (report only): Run on your GPU and report diffs without gating, to catch driver-specific issues.
- Skia's CPU renderer picks code paths by CPU feature level, and they differ in output. Which tier must match exactly?
  - Pin one tier, force it in oracle (Recommended): Force the oracle to a single tier (e.g. its AVX2 path) and match it exactly. Our other tiers must produce the same pixels.
  - Match every Skia tier: Reproduce SSE2, AVX2 and others with tier-specific behavior. Most faithful, by far the most work.
  - Scalar/baseline path only: Match Skia's baseline output. Simplest arithmetic to match, but the fast paths are never verified against Skia.

## User answered

The user answered: "Which Skia version is the oracle pinned to?"="Latest Skia release", "GPU oracle for exact-match tests? skia-safe has Graphite but no Dawn backend."="Skia dawn (latest)", "Reference GPU environment(s) whose output must match exactly (and gate CI)?"="Vulkan / lavapipe (Linux),D3D12 / WARP (Windows),Real hardware (report only)", "Skia's CPU renderer picks code paths by CPU feature level, and they differ in output. Which tier must match exactly?"="Match every Skia tier"

## Claude asked

- Which Skia modules count toward the '100% of Skia's tests' claim? (Anything left out is still allowed later, just not counted.)
  - Core + effects + CPU raster: Canvas, Paint, Path, Matrix, shaders, color filters, image filters, path effects, CPU rendering. The foundation; effectively required.
  - PathOps + GPU (Graphite): Boolean path operations, plus the wgpu backend modeled on Graphite.
  - Text + codecs + color management: SkFont/SkTypeface, glyph rendering, image decode/encode, skcms.
  - Modules: PDF, SVG, Skottie, Paragraph: Skia's higher-level modules (document output, SVG DOM, Lottie animation, rich text layout).
- Text rendering backend? Exact glyph pixels require using the same font engine as the oracle.
  - Fontations (skrifa) (Recommended): Skia ships a Fontations typeface backend built on Google's Rust skrifa crate. Configure the oracle to use it, and our glyphs can match exactly with no C font libraries.
  - Port FreeType behavior: Match Skia's FreeType backend by reimplementing FreeType rasterization in Rust. Huge effort.
  - Defer text: Keep text out until core and GPU are mature.
- SkSL (Skia's shader language, needed for SkRuntimeEffect and for GPU pipelines)?
  - Port Skia's SkSL compiler (Recommended): Faithful Rust port of the SkSL front end and WGSL code generator. Our WGSL then matches Skia's WGSL byte for byte, which is what makes exact GPU output achievable.
  - SkSL front end → naga IR: Parse SkSL, lower into naga, and let naga emit code. Less code, but generated WGSL won't match Skia's text, which makes exact-match debugging harder.
  - Precompiled shaders only: Ship Graphite's built-in shaders pre-translated and leave runtime effects unsupported at first.
- Image codecs?
  - Pure-Rust crates, matched to Skia (Recommended): png, zune-jpeg, image-webp, gif behind Skia's SkCodec API. Lossless formats match exactly; JPEG pixels checked separately because libjpeg-turbo's IDCT differs.
  - Port Skia's decoders bit-exact: Reimplement libjpeg-turbo/libwebp behavior so JPEG and WebP decode exactly like the oracle.
  - Out of scope: Users bring their own decoded pixels.

## User answered

Your questions have been answered: "Which Skia modules count toward the '100% of Skia's tests' claim? (Anything left out is still allowed later, just not counted.)"="Core + effects + CPU raster,PathOps + GPU (Graphite),Text + codecs + color management,Modules: PDF, SVG, Skottie, Paragraph", "Text rendering backend? Exact glyph pixels require using the same font engine as the oracle."="Fontations (skrifa) (Recommended)", "SkSL (Skia's shader language, needed for SkRuntimeEffect and for GPU pipelines)?"="Port Skia's SkSL compiler (Recommended)", "Image codecs?"="Pure-Rust crates, matched to Skia (Recommended)"

## Claude asked

- Where do the oracle's reference images live? (Thousands of GMs × CPU tiers × GPU environments adds up to GBs.)
  - Generate in CI, cache by hash (Recommended): Oracle binaries are built once per Skia pin and cached. Goldens are rendered on demand and cached, keyed by (Skia commit, GM, config). The repo stays small and always matches the pin.
  - Git LFS in the repo: Commit the PNGs. Easy offline diffs, but heavy repo churn on every Skia bump and LFS bandwidth limits.
  - Separate goldens repo/releases: Publish goldens as versioned release artifacts that the test harness downloads.
- How are Skia's C++ tests ported?
  - Faithful 1:1 translation (Recommended): Same names, same structure, same assertions, with a header comment linking the C++ source and line. Easy to review side by side and to re-sync on Skia bumps.
  - Idiomatic rewrite: Rust-style tests that cover the same behavior. Nicer to read, harder to prove equivalence.
  - Auto-transpile + hand fixup: Build a C++→Rust test translator for the macro-heavy patterns and hand-fix the rest.
- CI platforms to gate on (besides the GPU reference environments)?
  - Windows x64: Your platform; also exercises the D3D12/WARP environment.
  - Linux x64: Cheapest runners; also runs lavapipe.
  - macOS arm64: Covers NEON tiers and Metal via wgpu.
  - wasm32 + Linux aarch64: WASM build checks (WebGPU in browser later) and NEON CPU tiers on Linux ARM runners.
- Performance policy relative to Skia?
  - Track, don't gate (Recommended): Run benches against Skia in CI and publish trends. Correctness first; regressions over a threshold open an issue but don't fail PRs.
  - Gate at parity: Benches must be within X% of Skia or the PR fails. Strict, but blocks early progress while the code is still being ported.
  - Ignore until feature-complete: No perf tracking until correctness reaches 100%.

## User answered

The user answered: "Where do the oracle's reference images live? (Thousands of GMs × CPU tiers × GPU environments adds up to GBs.)"="I have a desktop server with an RTX 4070 SUPER where local tests can be run", "How are Skia's C++ tests ported?"="Faithful 1:1 translation (Recommended)", "CI platforms to gate on (besides the GPU reference environments)?"="Windows x64,Linux x64,macOS arm64,wasm32 + Linux aarch64", "Performance policy relative to Skia?"="Gate at parity"

## Claude asked

- How should the desktop server (RTX 4070 SUPER) fit in?
  - Self-hosted CI runner + golden store (Recommended): Register it as a GitHub Actions self-hosted runner. It builds and caches the oracle (Skia+Dawn), stores goldens on disk keyed by hash, and runs the full exact-match suites plus real-hardware reports. Hosted runners handle builds, lint and unit tests.
  - Local-only, manual runs: Developers run the oracle suite there by hand. CI only runs tests that need no oracle.
  - Runner + publish goldens: As the first option, but also upload the golden sets as release artifacts so contributors without the server can run comparisons.
- Parity threshold for the performance gate? (Same machine, same Skia CPU tier/backend, median of N runs.)
  - ≤ 10% slower (Recommended): Strict but leaves room for benchmark noise on shared hardware.
  - ≤ 5% slower: Very strict; needs a quiet dedicated machine (your server) to be reliable.
  - ≤ 0% (equal or faster): Must match or beat Skia on every bench.
  - Gate only after a feature is complete: Gate at 10% only for modules whose correctness tests are 100% passing; earlier modules just track.
- Text shaping and Unicode for Paragraph (Skia uses HarfBuzz + ICU)?
  - HarfRust + ICU4X (Recommended): HarfRust is the HarfBuzz team's official Rust port. Shaping output should match the oracle's HarfBuzz; ICU4X covers line breaking and BiDi.
  - rustybuzz + unicode-* crates: Older community HarfBuzz port; lighter but lags HarfBuzz behavior.
  - Port Skia's own shaper glue: Port SkShaper on top of a Rust HarfBuzz, plus a Rust ICU port. Most faithful, most work.
- How often is the Skia pin upgraded ('latest release' moves every ~4 weeks with each Chrome milestone)?
  - Every milestone (Recommended): Each new chrome/mNNN branch: bump the pin, regenerate goldens, diff the test inventory, and file issues for new/changed tests. Crate version tracks it, e.g. 0.156.x.
  - Every few milestones: Batch upgrades every 2–3 milestones; less churn, bigger diffs.
  - Only when stable: Upgrade only once the current milestone is at 100%.

## User answered

The user answered: "How should the desktop server (RTX 4070 SUPER) fit in?"="Local only + publish goldens", "Parity threshold for the performance gate? (Same machine, same Skia CPU tier/backend, median of N runs.)"="≤ 0% (equal or faster)", "Text shaping and Unicode for Paragraph (Skia uses HarfBuzz + ICU)?"="HarfRust + ICU4X (Recommended)", "How often is the Skia pin upgraded ('latest release' moves every ~4 weeks with each Chrome milestone)?"="Every milestone (Recommended)"

## Claude asked

- How is the implementation written, compared with Skia's C++?
  - Faithful port, idiomatic Rust (Recommended): Translate Skia's algorithms function by function with links to the C++ source, keeping the same arithmetic and operation order (needed for exact pixels), and expressed in safe, idiomatic Rust.
  - Clean-room from behavior: Implement from docs and test outputs without reading Skia source. Legally cleaner, but exact pixel matches become guesswork.
  - Hybrid: Faithful port for rendering math; free design for API, plumbing and memory management.
- Public API naming?
  - Drop Sk prefix, Rust casing (Recommended): SkCanvas::drawRect → skia_rust::Canvas::draw_rect, modules mirroring Skia's include/ layout (core, effects, gpu…), plus a doc alias for each original name so `SkCanvas` is searchable in docs.rs.
  - Same as skia-safe: Copy skia-safe's naming so users can switch with minimal edits.
  - Keep Sk prefix: SkCanvas, SkPaint… in Rust casing; closest to C++ docs, unusual in Rust.
- How should AI-driven ('vibecoded') work be organized?
  - Inventory-driven, small PRs (Recommended): Each task = one manifest entry (a Skia test/GM or source file). A CLAUDE.md sets the porting rules; every PR must flip manifest entries to 'passing' with the oracle check, with no test edits that loosen assertions.
  - Module milestones: Agents take whole modules (e.g. 'stroker') at once and land them behind a feature flag when its tests pass.
  - Mixed: Module-level design docs written first, then inventory-driven PRs within each module.
- When to publish to crates.io?
  - Reserve now, publish at core 100% (Recommended): Publish a placeholder now to hold the name, then the first real release once core + CPU raster pass 100% of their tests.
  - Publish 0.x early and often: Release each milestone with the current pass rate in the README.
  - Only at full 100%: No releases until every in-scope test passes.

## User answered

Your questions have been answered: "How is the implementation written, compared with Skia's C++?"="Faithful port, idiomatic Rust (Recommended)", "Public API naming?"="Drop Sk prefix, Rust casing (Recommended)", "How should AI-driven ('vibecoded') work be organized?"="Inventory-driven, small PRs (Recommended)", "When to publish to crates.io?"="Reserve now, publish at core 100% (Recommended)"

## Claude asked

- What OS does the RTX 4070 SUPER server run? It decides where the lavapipe (Linux) and WARP (Windows) oracle goldens get generated.
  - Windows: WARP natively; lavapipe via WSL2 or a Linux VM.
  - Linux: lavapipe natively; WARP goldens generated on your Windows dev machine instead.
  - Dual boot / VMs: Both environments are available on the server.
- What CPU is in the server? Skia's AVX-512 (SKX) tier needs AVX-512 hardware or Intel SDE emulation.
  - AMD Zen 4/5: Has AVX-512, so all x86 tiers run natively.
  - Intel 12th–14th gen / Core Ultra: No AVX-512; the SKX tier would run under Intel SDE (slow but exact).
  - Older / not sure: I'll plan for SDE and detect at runtime.

## User answered

The user answered: "What OS does the RTX 4070 SUPER server run? It decides where the lavapipe (Linux) and WARP (Windows) oracle goldens get generated."="Windows", "What CPU is in the server? Skia's AVX-512 (SKX) tier needs AVX-512 hardware or Intel SDE emulation."="Ryzen 7 7800x3D"

## Claude

The plan is written to [docs/PLAN.md](docs/PLAN.md). It records every decision you made and covers the crate layout, the oracle builds, what counts as a passing test, CI, the porting workflow, Skia upgrades, performance, releases, a phased roadmap and risks. Nothing has been built or tested yet; this is the plan only.

A few things in it are my own calls, not answers you gave:

- **CI checks hashes, not images.** Each golden's checksum is published in a small file, and CI only downloads the actual reference images when a test fails, to produce a diff. That keeps the gigabytes of goldens out of CI and out of the repo.
- **The oracle's GPU features are limited to what wgpu supports,** so Skia's Graphite takes the same code paths ours will. The generated WGSL must also be byte-identical to Skia's, and each shader is tested on its own before whole scenes are compared.
- **Every CPU tier gets its own oracle build.** Your 7800X3D is Zen 4, so it can run Skia's AVX-512 tier natively. Lavapipe goldens come from WSL2 on the server.
- **wasm32 is compared against a non-SIMD build of Skia,** since Skia has no separate WebAssembly tier.
- **The ≤ 0% performance gate needs a statistical test:** a benchmark fails only when it's slower *and* the difference is significant. Without that, run-to-run noise would block PRs at random.

The risks most likely to cause trouble later:

- **Exact GPU matches may not be achievable everywhere.** wgpu (through naga) and Dawn (through Tint) compile the same WGSL differently, so some shaders could produce different pixels.
- **Lossy JPEG/WebP and PDF output need bit-exact decoders and compression** before their tests can pass, which a pure-Rust crate alone won't give you.
- **GPU gating on hosted CI runners depends on lavapipe and WARP being deterministic across different CPUs.** That gets measured in Phase 0; if they aren't, GPU checks move to your server.

The directory isn't a git repository yet. Phase 0 starts with `git init`, the workspace skeleton and reserving the crate name.

## User

Regarding how the actual work gets done: Most of the agents can probably just be Sonnet 5.5 to save costs since it's a fairly straightforward port job. Higher level opus agents can be deployed only when a sonnet agent fails

Questions:

1. is there anything important that wgpu does not support?
2. why is wasm being compared against a non-SIMD build?
3. For lossy, may it be better to port Skia's decoders directly to avoid tuning thresholds later?

## Claude

I've added the Sonnet-first staffing to the plan as §8.3, with escalation to Opus after 2 failed attempts on the same task. Opus also handles a few areas from the start where a design mistake is expensive: module design notes, review of `unsafe` code, and structural changes when Skia is upgraded.

**1. What wgpu doesn't support**

Nothing that blocks correctness. The gaps are Dawn-only extensions that Graphite already treats as optional and works around when they're missing:
- **Framebuffer fetch.** Without it, Graphite reads the destination by copying it first.
- **MSAA render-to-single-sampled, transient attachments, load-resolve.** Without these, Graphite uses an extra MSAA texture and resolve. They cost memory bandwidth, not correctness.
- **Async pipeline creation.** Pipelines get created synchronously instead, which can cause stutter the first time a shader is used.
- **Multi-planar (video) texture formats.** Only partly supported in wgpu.
- In the browser, WebGPU's baseline also lacks dual-source blending and 16-bit normalized textures on some platforms, so more workarounds kick in there.

The one that matters for the plan is the ≤ 0% perf gate. Those extensions are exactly Graphite's GPU performance features, so the GPU benchmarks have to compare against the same capability-restricted oracle used for pixel tests, or parity is impossible by construction. I'll write that into §9.2.

**2. Why wasm was compared against a non-SIMD build**

That was a shortcut, and it doesn't fit "match every tier". Skia doesn't detect CPU features at runtime on wasm. CanvasKit, Skia's own WebAssembly build, compiles its vector code straight to WebAssembly's SIMD instructions (simd128). Those can produce different pixels than x86: for example, Skia uses x86's approximate reciprocal instructions where wasm has to do an exact divide. The right oracle is Skia built for wasm with Emscripten, the same way CanvasKit is, and run under Node, with two tiers: simd128 and no SIMD.

**3. Porting the lossy decoders directly**

Yes, and there were never going to be thresholds, since the plan has no tolerances anywhere. Without a port, the JPEG and WebP tests just stay failing. A correction, though: Skia doesn't have its own decoders. It wraps libjpeg-turbo (JPEG), libwebp (WebP) and Wuffs (GIF). For PNG it can use either libpng or the Rust `png` crate. So "porting Skia's decoders" really means porting the decode paths of those libraries. That's more tractable than it sounds:
- libjpeg-turbo's SIMD code for the default (`ISLOW`) inverse transform is designed to be bit-identical to its C code. libwebp's SIMD and C paths are also tested to match each other. So one faithful scalar port, sped up later with SIMD, covers every tier.
- It also covers Skia's codec tests on truncated and corrupted files. Those check decoder-specific behavior, like how much of a partial image gets decoded, which a generic Rust crate won't reproduce even for lossless formats.

## Claude asked

- wasm32 oracle?
  - Emscripten Skia, both tiers (Recommended): Build Skia with Emscripten the way CanvasKit is built (simd128 and no-SIMD), run oracle-dump under Node, and match both tiers exactly. Our wasm build runs under wasmtime.
  - simd128 tier only: Only match the SIMD build that browsers actually run; skip the no-SIMD tier.
  - Keep the non-SIMD shortcut: Compare wasm against the native portable build (SKVX_USE_SIMD=0). Simpler, but not faithful to what CanvasKit runs.
- Codec strategy?
  - Port each library Skia wraps (Recommended): Faithful Rust ports of the decode paths Skia uses: libjpeg-turbo, libwebp, Wuffs GIF, libpng behavior (or reuse the Rust png crate where Skia itself uses it). Exact pixels and exact behavior on truncated/corrupt files.
  - Port lossy only, crates for lossless: Port libjpeg-turbo and libwebp; use pure-Rust crates for PNG/GIF/BMP and accept possible mismatches on corrupt-file tests.
  - Upstream fixes to existing crates: Contribute bit-exact modes to zune-jpeg / image-webp instead of owning decoders. Less code to maintain, but we depend on upstream review speed.
- Which additional codecs are in scope? (Skia also supports these behind build flags.)
  - AVIF: Skia uses libavif + dav1d (AV1 decoding). A large port.
  - JPEG XL: Skia uses libjxl. Very large.
  - RAW (DNG): Skia uses piex + Adobe's dng_sdk. Large and rarely used.
  - Ultra HDR / gainmaps: JPEG with HDR gainmap metadata, on top of the JPEG port. Moderate.

## User answered

The user answered: "wasm32 oracle?"="simd128 tier only", "Codec strategy?"="Port each library Skia wraps (Recommended)", "Which additional codecs are in scope? (Skia also supports these behind build flags.)"="[No preference]"

## Claude

The wasm, codec and GPU-benchmark decisions are now in [PLAN.md](docs/PLAN.md). Extra codecs (AVIF, JPEG XL, RAW) default to whatever Skia's standard build enables, and the rest are excluded with a reason until you revisit them.

**Getting the performance features back**

Most of them matter much less than it sounds. They speed up **tile-based GPUs**: Apple Silicon, phones, and some integrated GPUs. On a desktop card like your RTX 4070 they're either unsupported or barely help, so losing them costs little where the perf gate actually runs. Here's each one:

| Feature | How to get it | Effort |
|---|---|---|
| Async pipeline creation | Needs nothing from wgpu. Its device can be used from several threads, so we compile pipelines on background threads ourselves and use wgpu's `PipelineCache` on Vulkan. Matters on every GPU, because it removes first-use stutter. | Small, ours |
| Transient (memory-free) attachments | Add a native-only feature upstream in wgpu; backends use Metal memoryless storage and Vulkan lazily allocated memory. | Medium, upstream |
| MSAA render-to-single-sampled, load-resolve | Add native-only features upstream; Vulkan has an extension for this, and Metal can emulate it with memoryless MSAA. | Medium, upstream |
| Framebuffer fetch | Needs both a WGSL language extension in naga (matching Dawn's experimental one) and backend work in wgpu: Metal programmable blending, Vulkan input attachments. D3D12 has no equivalent. | Large, upstream |

**Options I'd rule out:**
- **Calling Vulkan, Metal or D3D12 directly through wgpu's escape hatch (`as_hal`).** It needs `unsafe` per backend in the GPU crate, which breaks your unsafe policy, and it's exactly the kind of sloppy shortcut you want to avoid.
- **Forking wgpu.** Every wgpu release would then mean a painful rebase.

**Testing them without loosening exactness:** each feature we enable changes which Graphite code path runs. So each one needs an extra oracle configuration with the same feature turned on in Dawn, and matching is only checked on hardware that supports it. lavapipe and WARP probably don't support most of these. That means Apple Silicon becomes the reference machine for those paths, but GitHub's macOS runners are virtual machines with limited GPU support, so it may need a physical Mac.

## Claude asked

- How should the plan handle these GPU performance features?
  - Async pipelines now, upstream later (Recommended): Background pipeline compilation + PipelineCache in Phase 6. After GPU exact-match is done, contribute transient attachments → MSAA-to-single-sampled → framebuffer fetch to wgpu/naga as native-only features, each with a matching feature-on oracle config.
  - Upstream in parallel with Phase 6: Start the wgpu/naga contributions while Graphite is being ported, so the features land closer to when they're needed. More concurrent work.
  - Async pipelines only: Skip the tile-GPU features entirely; desktop parity is the target.
- Reference hardware for feature-on paths (Apple/tile GPUs)?
  - Decide when we get there: Leave it as an open item until the upstream features exist.
  - Physical Mac (Apple Silicon): A Mac you own or buy acts as a second report/gating machine for Metal feature paths.
  - Report-only, no gating: Feature-on paths are checked against the oracle when someone runs them on suitable hardware, but never gate merges.

## User answered

Your questions have been answered: "How should the plan handle these GPU performance features?"="Async pipelines only", "Reference hardware for feature-on paths (Apple/tile GPUs)?"="Decide when we get there"

## Claude

I've recorded your two answers in [PLAN.md](docs/PLAN.md):

- **GPU performance:** only async pipeline compilation (on background threads, plus wgpu's `PipelineCache`), now part of Phase 6. The Dawn-only features for tile-based GPUs aren't pursued; desktop parity is the target.
- **Reference hardware for those features:** left as an open item, to decide only if they're ever picked up.

All decisions so far are written into the plan. The next step is Phase 0: `git init`, the workspace skeleton, the lint configuration and reserving the crate name.

## User

Let's continue

## Claude

Latest Skia release branch is `chrome/m156` (`95ee33d7`). Cloning it in the background for the inventory tool while I scaffold.

## Claude

Skia m156 cloned. Writing the crate skeletons.

## Claude

Now the inventory. Surveying Skia's test macros:

## Claude

Workspace and crate skeletons are in place. I'm now building the `xtask` inventory tool that lists every Skia test.

## Claude

8,275 entries generated. Checking for test macros the scanner misses:

## Claude

Skia also registers tests through its own wrapper macros (`UNIX_ONLY_TEST`, `SHAPER_TEST`, `MAKE_GM`, …). The scanner will now treat any macro that expands to a registration macro as one itself:

## Claude

Escaping got lost through the heredoc; fixing directly.

## Claude

Whitespace only; collapsing it in `collect_defines`.

## Claude

PathOps jumped from 125 to 1,609, which looks wrong. Checking which macro caused it:

## Claude

The counts now look right: 8,597 entries, 6,329 in scope. The 382 excluded unit tests match the number of Ganesh-only macro uses. Next: running clippy, then repository files and CI.

## Claude

Clippy pedantic is clean and the tests pass. Now the license and repository files:

## Claude

Now the CI workflow:

## Claude

I'm fixing a line-ending escape that the shell mangled in the inventory tool. After that comes a final check, then a summary.

## Claude

Fixed; clippy and the tests are clean. Final touches:

## Claude

The local part of Phase 0 is in place. `cargo xtask inventory sync` scans Skia `chrome/m156` (commit `95ee33d7`) and lists every test at that commit: 8,597 entries, of which 6,329 are in scope. Clippy pedantic, fmt and the xtask tests are clean. Nothing ports or renders yet, and the CI workflow hasn't run since nothing is pushed.

| Kind | Total | Excluded | In scope |
|---|---:|---:|---:|
| Unit tests | 2,954 | 382 | 2,572 |
| GMs | 1,112 | 17 | 1,095 |
| Benchmarks | 1,280 | 0 | 1,280 |
| Fuzzers | 32 | 0 | 32 |
| SkSL compiler outputs | 2,793 | 1,869 | 924 |
| DM image decodes | 236 | 0 | 236 |
| DM Lottie renders | 190 | 0 | 190 |

The exclusions all follow from plan decisions, and each has its reason written in the manifest:
- **Ganesh-only tests:** Graphite is the only GPU backend in scope.
- **Metal/Vulkan/GL backend internals:** our GPU backend is wgpu.
- **Tests disabled upstream** in Skia itself.
- **Expected outputs for SkSL compilers we don't port** (GLSL, Metal, HLSL, SPIR-V). We only port the WGSL, Raster Pipeline, SkSL and pipeline-stage compilers.

One addition you didn't ask for: Skia ships expected outputs for its SkSL compiler (420 WGSL and 365 Raster Pipeline files). They let us check the SkSL port byte for byte without needing the oracle, so they're in the inventory.

What's in the repo:
- **Workspace:** facade, `skia-rust-simd` (the only crate allowed `unsafe`) and `skia-rust-core`, with the lints and versioning from the plan.
- **`xtask`:** `cargo xtask skia fetch` clones the pinned Skia into `third_party/skia`. `cargo xtask inventory sync` rescans it and updates the manifest while keeping the module/status/reason fields you edit by hand; any ported test whose Skia source changed is marked `stale`. `cargo xtask inventory stats` prints pass rates.
- **Agent and policy docs:** [CLAUDE.md](CLAUDE.md) holds the porting rules and the Sonnet→Opus escalation, alongside [docs/UNSAFE.md](docs/UNSAFE.md), the BSD-3 [LICENSE](LICENSE) with Google's notice, and the [README](README.md).
- **CI** in [.github/workflows/ci.yml](.github/workflows/ci.yml): fmt, clippy, the unsafe-policy check, `cargo deny`, tests on 4 operating systems, a wasm32 simd128 build, Miri, and a check that the manifest matches the Skia pin.

I ran `git init`, but nothing is committed or pushed. The rest of Phase 0 needs you or the server:
1. **Commit, and create the GitHub repo** under `CoolBeansStudioOfficial`. Say if you want me to do both.
2. **Reserve the crate name** with a placeholder `skia-rust 0.0.0` publish. That needs your crates.io token, so you'll have to run it yourself.
3. **Oracle on the server:** build Skia with Dawn, Graphite and Fontations for every CPU tier, set up WSL2 for lavapipe, and write the `oracle-dump` tool. I can write the build scripts and the C++ tool here, but you'll need to run them on the server.

## User

Let me move this session to the server

## Claude

I can't move this session to another machine myself. The only move I have is to Anthropic's cloud, not your server. The usual route is to push the repo to GitHub, clone it on the server, and start a new Claude Code session there. [CLAUDE.md](CLAUDE.md) and [docs/PLAN.md](docs/PLAN.md) carry the project context, and I can also export this conversation into the repo so the new session can read it.

Creating the repo and pushing go out to GitHub, so I need your go-ahead first:

## Claude asked

- Create the GitHub repo CoolBeansStudioOfficial/skia-rust and push the initial commit?
  - Yes, private repo (Recommended): Commit everything, create a private repo with gh, push main. Make it public later when there's something to show.
  - Yes, public repo: Same, but public from the start. CI minutes are free for public repos.
  - Commit only, I'll push: Make the initial commit locally; you create the repo and push.
- Carry this conversation's context to the server session?
  - Export transcript to notes/ (Recommended): Save this session's transcript as notes/session-001-planning.md in the commit so the server session can read the reasoning behind decisions.
  - No, PLAN.md is enough: The plan and CLAUDE.md already record every decision.

## User answered

Your questions have been answered: "Create the GitHub repo CoolBeansStudioOfficial/skia-rust and push the initial commit?"="Yes, public repo", "Carry this conversation's context to the server session?"="Export transcript to notes/ (Recommended)"

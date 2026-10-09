# Design: Benchmarks and the performance gate (PLAN §9.2)

Status: proposed (2026-10-09). Pin: `chrome/m156` (`95ee33d7ec77`). Audience: every agent porting
a file from Skia's `bench/` (`DEF_BENCH` and its wrappers), the agent building the bench harness
and the comparison tooling, and the maintainer, who has to decide how the perf gate works now that
the oracle server is gone.

Skia references are `path#Lx-Ly` in the pinned tree (`third_party/skia`). Manifest counts are from
`inventory/manifest.toml` at `cecde7b`. Golden facts come from `hashes-m156.json` of release
`goldens-m156` (`inventory/goldens.lock`). "Hosted runner" means the GitHub-hosted runners CI
already uses: `ubuntu-latest` (x64), `windows-latest`, `macos-latest` (arm64) and
`ubuntu-24.04-arm`. The repository is public, so standard hosted runners cost no minutes, but
the CI pool is already busy (12 jobs per PR).

## Decisions at a glance

1. **The manifest has 1,280 `bench` entries, all `todo`.** By module: core 819, effects 301,
   text 42, codec 35, svg 32, skcms 19, pdf 12, sksl 9, pathops 8, gpu 2, paragraph 1. The module
   heuristic mislabels about 240 of them (§1.2). B0 relabels them: the 32 `svg` entries and 18
   others are Skottie, 40 are GPU-shared code, and 34 are the shaper. **111 test Ganesh internals
   and are proposed for exclusion** (Q7), which leaves **1,169 in scope**.
2. **A manifest entry is a registration site, not a benchmark.** The id is
   `<file>::<the DEF_BENCH expression>`, for example
   `bench/PathBench.cpp::ArbRoundRectBench(false)` or `bench/ShaperBench.cpp::armenian`. One entry
   can create several runtime benchmarks (`COMPILER_BENCH` expands to 6 `DEF_BENCH`es,
   `ADD_BENCH_FAMILY` to a template family), and each benchmark runs in every config it is
   suitable for. The port registers each site under the manifest name **verbatim**, and the
   harness tags every result with that id (§2.3).
3. **Ports live in a new workspace crate `skia-rust-bench` at `benches/`** (the directory PLAN
   §3.1 already reserves). It has one module per Skia file (`bench/PathBench.cpp` →
   `benches/src/bench/path_bench.rs`), a `Benchmark` trait that mirrors `bench/Benchmark.h`
   method by method, a `def_bench!` registry on the `inventory` crate (as in `skia-rust-gm`),
   and **a port of nanobench's CPU path** as the runner (§2, §4).
4. **Custom runner, not criterion** (§4). Skia's benchmarks own their inner loop
   (`onDraw(loops, canvas)`) and their per-canvas setup, and the comparison needs the same loop
   counts, configs, sample definition and JSON on both sides. A ported nanobench gives all of
   that, writes nanobench's own `--outResultsFile` JSON, and adds no dependencies.
5. **`passing` means "ported and runs correctly". Perf parity is a separate verdict** (Q2). A
   bench entry is `passing` when every benchmark it registers (a) has Skia's name, size, units
   and backend suitability, (b) runs one smoke iteration on every native CI platform without a
   panic or failed debug assertion, and (c) for rendering benchmarks, leaves a canvas whose bytes
   equal Skia's after the same smoke iteration, where a bench golden exists (§3). Perf results
   go into a separate ledger and never flip manifest status.
6. **`goldens-m156` has nothing for benches.** All 28 tiers hold `gm` results only (2,727
   results per x64 CPU tier, 909 per RGBA tier, 910 per GPU tier). Criterion (c) therefore needs
   new oracle data. That data comes from **nanobench built at the pin on a hosted Linux x64
   runner** (Q1), the same build the perf comparison needs. It is run with `--loops 1 --samples
   1` and a raw-dump flag added to the oracle patch. Until those dumps exist, rendering benches
   that pass (a) and (b) stay `ported`, and non-rendering benches can be `passing` (Q3).
7. **The perf comparison runs both implementations in the same hosted-runner job** (Q1, Q5).
   The job builds Skia's `nanobench` once per pin (cached), builds our `nanobench` in release,
   and runs the two **interleaved** in rounds, with the same loop counts, the same CPU tier
   (`SKIA_ORACLE_CPU_CAP` on Skia's side, `CpuCap` on ours), the same allocator, a pinned core,
   and warm-up samples dropped (§5). Comparing the two inside one VM cancels most of the
   noise of shared hardware. Comparing across jobs or against stored numbers does not.
8. **The statistic stays PLAN's (one-sided Mann-Whitney U, p < 0.05). For hosted runners it
   gains two things:** a noise margin δ, measured by A/A runs (task B5), and Benjamini-Hochberg
   control across the roughly 1,500 comparisons of a sweep. A "slower" verdict is also re-run
   once in a fresh job before it counts (Q4). With δ = 0 and no FDR control, an A/A sweep of
   1,500 comparisons would be expected to report about 75 false "slower" verdicts.
9. **Gate on `ubuntu-latest` x64 at the `ml3` tier, configs `nonrendering` and `8888`**
   (nanobench's own defaults minus `gl`). arm64 Linux (Neon), macOS arm64 and Windows report
   only, at first (Q5).
10. **Cadence: a nightly full sweep, sharded, plus a PR run triggered by a `perf` label** on a
    canary set and on the benches whose ports the PR touched. It is advisory until B5 has fixed
    δ, then required for labelled PRs (Q6). Results go to a `perf-data` branch as a time series
    for the dashboard (Q11).
11. **Work order (§7):** bookkeeping (B0) → harness and verify plumbing (B1, B2) → module
    sweeps of benches whose code is already ported (B10–B19, about 950 entries, can start at
    once) → the CI nanobench build, bench goldens and comparison tooling (B3, B4) → calibration
    and the perf workflow (B5, B6) → module benches that wait on other phases (B20–B27).

---

## 1. What Skia's benches are

### 1.1 The `Benchmark` API (`bench/Benchmark.h#L42-L135`, `bench/Benchmark.cpp`)

| C++ | Meaning |
|---|---|
| `onGetName()` / `onGetUniqueName()` | Result name (unique name defaults to the name). nanobench's JSON key is `<uniqueName>_<w>_<h>` (`bench/ResultsWriter.h`, `beginBench`). |
| `onGetSize()` | Canvas size, default 640×480. |
| `isSuitableFor(Backend)` | Default: everything except `kNonRendering`. Backends: `kNonRendering`, `kRaster`, `kGanesh`, `kGraphite`, `kPDF`, `kHWUI`. |
| `shouldLoop()` | `false`: run with `loops = 1` regardless of calibration. |
| `onDelayedSetup()` | Called once per benchmark, before any config, outside the timer. |
| `onPerCanvasPreDraw` / `onPerCanvasPostDraw` | Once per (benchmark, config), outside the timer. |
| `onPreDraw` / `onPostDraw` | Around each timed `draw`, outside the timer. |
| `onDraw(int loops, SkCanvas*)` | The timed body. It loops `loops` times itself. **The canvas is null for `kNonRendering`.** |
| `submitsInternalFrames` / `onDrawFrame` | Multi-frame benches (MSKP only, not in the manifest). |
| `setUnits(n)` | Samples are divided by `n` (time per unit). |
| `setupPaint(SkPaint*)` | Default: anti-alias on. |
| `draw(loops, canvas, submit)` | Wraps `onDraw` in `SkAutoCanvasRestore`. |

`DEF_BENCH(code)` registers `[](void*) -> Benchmark* { code; }` in a `sk_tools::Registry`
(`Benchmark.h#L23-L26`).

### 1.2 What the manifest contains

The scanner (`xtask/src/inventory.rs`, `scan`/`normalize_name`) records each use of a
registration macro as `<rel path>::<first argument>`. It strips `return`, `new ` and a trailing
`;`, and appends `#n` for the n-th repeat of a name in one file. For benches the first argument is
the whole factory expression. The ids are therefore C++ text, sometimes with comments and line
breaks folded in, for example
`bench/DecodeBench.cpp::SkottieDecodeBench("skottie_large", // 426593 "skottie/skottie-text-scale-to-fit-minmax.json")`.
Only one id repeats (`bench/BulkRectBench.cpp::1000#2`), and 413 ids are longer than 90
characters.

| Macro | Entries | Runtime benchmarks per entry |
|---|---|---|
| `DEF_BENCH` | 1,188 | 1 |
| `SHAPER_BENCH` | 34 | 1 |
| `BENCH` (local wrappers in `BlendmodeBench`, `RotatedRectBench`, …) | 29 | 1 |
| `DEF_PATH_TESS_BENCH` | 8 | 1 |
| `DEF_BOUNDS_MANAGER_BENCH_SET` | 6 | several |
| `COMPILER_BENCH` | 4 | 6 (one per SkSL output) |
| `DEF_FOR_AA_MODES` / `DEF_FOR_PERSP_MODES` | 3 / 2 | several |
| `ADD_BENCH_FAMILY` | 2 | a template family |
| `GRAPHITE_BENCH`, `DEF_CHECKSUM_BENCH`, `PARAGRAPH_BENCH` | 2, 1, 1 | 1–2 |

**Module labels.** `guess_module` matches path keywords, so Skottie files whose names contain
`svgo` became `svg`, and the shaper and GPU-shared benches fell through to `core`. Proposed
relabels (B0, `module` is hand-editable):

| From → to | Entries | Which |
|---|---|---|
| svg/core/text/effects → `skottie` | 50 | `DecodeBench.cpp::Skottie{,Picture}DecodeBench(...)` |
| core → `gpu` | 40 | `bench/graphite/**` (28), `RectanizerBench` (6), the six `TessellateBench` entries on `src/gpu/tessellate` (Wang's formula, middle-out) |
| core/text → `shaper` | 34 | `ShaperBench` |
| gpu → `sksl` | 2 | `SkSLBench` `graphite_small`/`graphite_large` (SkSL→WGSL compile time; they stay with G19 in `gpu.md` for the Graphite half) |
| core → `paragraph` | 1 | `bench/ParagraphBench.cpp` |
| skcms → `core` | 2 | `FindCubicConvex180ChopsBench` |

**Ganesh internals (Q7).** The scanner's policy (`policy_exclusion`) only excludes macros whose
names say `GANESH`, so Ganesh benches registered with plain `DEF_BENCH` are still `todo`. 111
entries test Ganesh code (`src/gpu/ganesh/**`) or run only on `Backend::kGanesh`:
`CompositingImagesBench` 34, `GrMemoryPoolBench` 22, `GrResourceCacheBench` 20,
`ImageCacheBudgetBench` 10, `GrMipmapBench` 4, `VertexColorSpaceBench` 4, `TriangulatorBench` 3
(`GrTriangulator`), `TessellateBench` 5 (`GrPath*Tessellator`, `TessPrepareBench` on
`GrMockOpTarget`), `GrQuadBench` 2, `CreateBackendTextureBench` 2, `TopoSortBench` 1
(`GrTTopoSort`), `DDLRecorderBench` 1, `GlyphQuadFillBench` 1, `ImageBench` 1, `ImageCycleBench`
1. They fall under the same "Ganesh-only; Graphite is the only GPU backend in scope" reason as the
excluded unit tests. (`text.md` T26 lists `GlyphQuadFillBench`. It is `kGanesh`-only, so it
belongs here.)

**Infrastructure Rust replaces (Q8).** `RefCntBench` (6: `SkRefCnt`, `SkWeakRefCnt`) and
`MutexBench` (4: `SkMutex`, `SkSharedMutex`, `SkSpinlock`) time types that the port replaces with
`Arc`, `Weak`, `Mutex` and `RwLock` (CLAUDE.md rule 3). They can be ported as benches of the
replacements, which is informative but only report-only (the recommendation), or excluded.

**Backend mix.** A static scan of `isSuitableFor` overrides (approximate; B0 records the exact
split from a nanobench `--dryRun`): about 380 entries are `kNonRendering` only, about 690 use the
default (every rendering backend), 4 are `kRaster` only, 58 are `kGanesh` only (all among the 111
above), 8 are `kGraphite` only, and about 140 inherit their suitability from a base class or a
macro. In scope, that is **roughly 400 non-rendering and 770 rendering entries**.

### 1.3 How nanobench times a CPU benchmark (`bench/nanobench.cpp`)

1. `estimate_timer_overhead()` (`#L439-L446`) averages 100,000 empty `now_ms()` pairs.
2. Per benchmark: `delayedSetup()` once (`#L1540-L1542`). Per config: `is_enabled` (`#L752`)
   checks `isSuitableFor` and makes a `Target`. For `kRaster` that is
   `SkSurfaces::Raster(SkImageInfo::Make(bench->getSize(), color, alpha, colorSpace))`
   (`#L249-L256`). Then `perCanvasPreDraw`.
3. `setup_cpu_bench` (`#L503-L545`) times `loops = 1` until it exceeds the overhead (at most
   `--maxCalibrationAttempts` 3 tries), then sets
   `loops = ceil((overhead/overheadGoal − overhead) / (bench − overhead))`, clamped to
   `[1, --maxLoops]`. With the default `--overheadGoal 0.0001`, **each sample lasts only about
   overhead × 10⁴, a fraction of a millisecond.** `shouldLoop() == false` forces 1.
4. Warm-up: the first benchmark of the run alone is timed repeatedly for 1 s (`#L1587-L1595`),
   and only when loops are auto-tuned.
5. `--samples` (default 10) calls of `time()` (`#L421-L437`). Each one clears the canvas to white,
   then `preDraw`, then the timed `draw(loops)`, then `postDraw`. The sample is
   `elapsed / loops / units` (`#L1605-L1615`).
6. JSON (`#L1648-L1665`): `results.<name>_<w>_<h>.<config>` = `{options: {name, source_type,
   bench_type}, min_ms, min_ratio, samples: [...]}`. `Stats` (`tools/Stats.h`) takes the median
   as `sorted[n/2]`.

CPU configs (`#L691-L720`): `nonrendering`, `a8`, `gray8`, `r8`, `565`, `8888` (N32), `rgba`,
`bgra`, `f16`, `srgba`. The default `--config` is `"8888 gl nonrendering"`
(`tools/flags/CommonFlagsConfig.cpp#L33-L34`). nanobench also benchmarks GMs, SKPs, SVGs and
images through `BenchmarkStream` (`#L830-L970`). None of those are manifest entries (Q12).

`--nativeFonts` defaults to **true** (`tools/fonts/FontToolUtils.cpp#L92-L94`), so text benches
must be run with `--nativeFonts false` to use the portable test fonts that the DM goldens use
(`oracle/README.md`, Fonts).

---

## 2. Porting a bench

### 2.1 Crate and layout

```
benches/                          # crate skia-rust-bench (publish = false)
├─ Cargo.toml                     # deps: skia-rust-{core,raster,effects,text,…}, skia-rust-tools,
│                                 #   inventory; feature "check" adds skia-rust-gm (goldens, tiers)
├─ src/lib.rs                     # Benchmark trait, Backend, def_bench!, registry, prelude
├─ src/nanobench/                 # port of nanobench's CPU path (§4)
│   ├─ mod.rs  config.rs  target.rs  stats.rs  results_writer.rs  timer.rs
├─ src/bench/<file_snake>.rs      # one per bench/<File>.cpp, e.g. path_bench.rs
├─ src/bench/graphite/…           # bench/graphite/**
├─ src/modules/skparagraph/paragraph_bench.rs
├─ src/check.rs                   # smoke + golden check (feature "check")
└─ src/bin/{nanobench,bench-verify}.rs
```

The crate is host tooling. Like `skia-rust-gm`, it is excluded from the wasm32 build and from
Miri, because it reads resources from disk. It is a workspace member, so clippy pedantic and
`RUSTFLAGS=-D warnings` apply to it. `<file_snake>` follows `verify::snake_case`
(`xtask/src/verify.rs`): `PathBench` → `path_bench`, `SkSLBench` → `sk_sl_bench`.

### 2.2 The trait

No skia-safe equivalent exists, so names follow rule 4 (drop `Sk`, Rust casing, `doc(alias)`).
The methods mirror `Benchmark.h` one for one, in the style of the `GM` trait in
`tests/gm/src/lib.rs`:

```rust
pub enum Backend { NonRendering, Raster, Graphite, Pdf }   // Ganesh and HWUI dropped

pub trait Benchmark {
    #[doc(alias = "onGetName")]        fn name(&self) -> String;
    #[doc(alias = "onGetUniqueName")]  fn unique_name(&self) -> String { self.name() }
    #[doc(alias = "onGetSize")]        fn size(&mut self) -> ISize { ISize::new(640, 480) }
    #[doc(alias = "isSuitableFor")]    fn is_suitable_for(&self, b: Backend) -> bool { b != Backend::NonRendering }
    #[doc(alias = "shouldLoop")]       fn should_loop(&self) -> bool { true }
    #[doc(alias = "getUnits")]         fn units(&self) -> i32 { 1 }
    #[doc(alias = "onDelayedSetup")]   fn on_delayed_setup(&mut self) {}
    #[doc(alias = "onPerCanvasPreDraw")]  fn on_per_canvas_pre_draw(&mut self, _c: Option<&Canvas>) {}
    #[doc(alias = "onPerCanvasPostDraw")] fn on_per_canvas_post_draw(&mut self, _c: Option<&Canvas>) {}
    #[doc(alias = "onPreDraw")]        fn on_pre_draw(&mut self, _c: Option<&Canvas>) {}
    #[doc(alias = "onPostDraw")]       fn on_post_draw(&mut self, _c: Option<&Canvas>) {}
    #[doc(alias = "onDraw")]           fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>);
    #[doc(alias = "setupPaint")]       fn setup_paint(&self, paint: &mut Paint) { paint.set_anti_alias(true); }
}
```

- `loops` stays `i32`, as in the C++ `int`. A negative value means "forever" (`SK_MaxS32`).
- The canvas is `Option<&Canvas>`, because nanobench passes null for `kNonRendering` and
  benches rely on that. A bench that dereferences it unconditionally in C++ uses `.expect()`
  with the C++ precondition as the message.
- `setUnits` becomes a field that the bench returns from `units()`. `draw()` (the
  `SkAutoCanvasRestore` wrapper) is a free function in the runner, not a trait method.
- Bench member variables become struct fields. Static helpers become private snake-cased `fn`s.
  `SkRandom`, `ToolUtils` and resources come from `skia-rust-tools`, as for GMs.

### 2.3 Registration

```rust
// Port of: bench/PathBench.cpp#L1212-L1212 (chrome/m156)
def_bench!(ArbRoundRectBench_false = "ArbRoundRectBench(false)", ArbRoundRectBench::new(false));
// A wrapper macro is ported as a macro_rules! that expands to def_bench! with the argument
// stringified (`SHAPER_BENCH(armenian)` → name "armenian").
// A registration that creates several benchmarks (COMPILER_BENCH, ADD_BENCH_FAMILY):
def_bench_set!(large = "large", vec![Box::new(SkSlCompileBench::new("large", LARGE_SRC, false, Output::None)), …]);
```

- **The name string is copied verbatim from the manifest id after `::`**, with the comments and
  whitespace exactly as the scanner folded them. Never retype it. `xtask inventory module-path`
  gets a bench mode that prints the Rust path and the name literal for an id.
- The registry key is `bench::<file_snake>::<name>`, or `modules::skparagraph::<file_snake>::<name>`
  for module benches. `verify` maps it back to the manifest id the same way it does for GMs
  (`xtask/src/verify.rs`, `module_path`, extended for the `bench/` prefix).
- Each registration also generates a `#[test]` (the smoke check of §3.1), so
  `cargo test -p skia-rust-bench <name>` runs one bench.
- A runtime benchmark the port omits, because it needs an out-of-scope backend (the GLSL, Metal
  and SPIR-V outputs of `COMPILER_BENCH`, `kGanesh`-only branches), is listed in the
  registration as `omitted: &["sksl_large_glsl", …]` with the reason in a comment. The name
  parity check (§3.1) accepts exactly those absences.

### 2.4 Rules that are specific to benches

- **Port the work, not just the name.** The comparison is meaningful only if both sides do the
  same work per loop: the same geometry, the same paint, the same number of draws, the same
  `SkRandom` sequence. Arithmetic rules (CLAUDE.md rule 2) apply as everywhere.
- **Nothing hoisted, nothing added.** Do not move work from `on_draw` into setup or the other way,
  and do not add `black_box` where Skia has no equivalent. Where Skia defeats the optimizer (a
  `volatile` sink, `sk_ignore_unused_variable`, a `fSum += …` member), use `std::hint::black_box`
  in the same place and say so in a comment.
- **Allocation stays where it is.** A per-loop `new`/`delete` in C++ is a per-loop
  `Box`/`Vec` in Rust. The comparison runs both sides on the platform `malloc` (§5.2), so the
  allocator is not a variable.
- Debug assertions (`SkASSERT`) become `debug_assert!`, so the smoke tests check them.

---

## 3. What makes a bench entry `passing` (Q2, Q3)

### 3.1 Correctness criteria (manifest status)

| # | Criterion | Applies to | Needs |
|---|---|---|---|
| C1 | **Registration parity.** The entry's runtime benchmarks have the unique names, sizes, units, `should_loop` and backend suitability of Skia's, minus the declared omissions | all | `bench-names-m156.json` (B3); before it exists, review against the C++ |
| C2 | **Smoke run.** In each config the benchmark is suitable for (`nonrendering`, `8888`, `565`, `f16`), the runner performs `delayedSetup`, `perCanvasPreDraw`, `preDraw`, `draw(1)`, `postDraw` and `perCanvasPostDraw` with no panic and no failed `debug_assert!`, in debug and release, on the four native CI platforms | all | nothing |
| C3 | **Output parity.** After the C2 sequence on a white-cleared canvas, the canvas bytes equal Skia's for every tier with a bench golden, selected with `skia_rust_simd::testing::oracle_selection` as for GMs | rendering benches, `8888` (and `565`/`f16` where Q3 adds them) | bench goldens (B3) |

`passing` = C1 and C2, plus C3 for rendering benches. With `--loops 1 --samples 1` nanobench
neither calibrates nor warms up (both happen only with auto-tuned loops, §1.3), so its canvas after
one sample is the deterministic state C3 compares. Benches whose state carries over between
samples (an `SkRandom` member, a growing cache) are fine for the same reason: both sides run
exactly one sample.

**Before bench goldens exist** (B3 not done, or Q1 declined), a rendering bench that meets C1
and C2 is set to `ported`. The status already exists and counts as ported for stale detection
(`inventory.rs#L108-L111`). `bench-verify --update` promotes it to `passing` once its goldens
appear. Non-rendering benches have no output to compare and can be `passing` on C1 and C2
alone. Q3 asks whether rendering benches may also pass on C1 and C2 if bench goldens are never
built.

**Why C3 is worth having.** A bench port that draws the wrong thing still runs, and then the
perf comparison compares different work. C3 is the only check that the measured work is the same.
It also adds, almost free, about 770 extra pixel checks of the drawing stack on canvases and
paint combinations the GM suite does not cover.

### 3.2 Not part of `passing`: the perf verdict

Perf parity is recorded per (manifest id, runner class) in the perf ledger (§5.5) as `parity`,
`slower`, `not-comparable` (the Skia side cannot run it, for example omitted outputs) or
`unmeasured`. It never changes manifest status, for two reasons:

- Many correct ports will be slower for a long time (the raster pipeline interpreter, issue #36,
  is the expected first cause). Tying `passing` to speed would hide correctness progress and
  invite "optimizations" that break exactness.
- Perf verdicts come from shared hardware and are statistical. Manifest status must be
  reproducible by `cargo xtask inventory verify` on any host.

PLAN §1 goal 5 ("each benchmark equal to or faster than Skia") then becomes a release criterion
read from the ledger, rather than a test status.

### 3.3 Verify plumbing (B2)

`inventory verify` gains a third leg after units and GMs. `bench-verify` (release, like
`gm-verify`, `a67452d`) runs C1–C3 for every registration and prints one line per registration,
and `verify_benches::check` compares that with the manifest. Rules mirror the GM rules: a
registration that is not checkable on a host (no golden of the host's N32 order, a tier without
its estimate fingerprint) keeps its status there and is never a regression. `#[ignore]` needs a
`failing` status and a reason, as everywhere.

---

## 4. The harness: a nanobench port, not criterion

| | criterion / divan | Ported nanobench (proposed) |
|---|---|---|
| Inner loop | The harness owns the loop. `iter_custom` can hand it to the bench, but calibration and sampling stay criterion's | `onDraw(loops)` as in Skia. Loop calibration, overhead estimate, warm-up and sample definition are Skia's, ported |
| Per-canvas setup, configs, `isSuitableFor`, units | To be built on top | Ported (`Target`, `CPU_CONFIG`) |
| 1,500 benchmarks in one binary | One `[[bench]]` target per file, or a giant group. Slow to build and to run | One binary, registry-driven, `--match` regexes as in nanobench |
| Output | criterion's JSON. Not comparable with Skia without a translation layer that has to guess loops and units | **nanobench's `--outResultsFile` JSON**, so one comparator reads both sides |
| Statistics | Bootstrap and linear regression on its own sample model | Raw samples out. Statistics live in `xtask bench compare` (§5.4), the same for both sides |
| Dependencies | criterion pulls plotters, rayon, serde and ciborium into `cargo deny` | none new (`serde_json` already in the workspace) |
| Rule 1 ("port, don't invent") | A different measurement method | The measurement method *is* the port |

The port covers `nanobench.cpp`'s CPU path only: `Config`/`Target` for `nonrendering` and the
raster configs, `setup_cpu_bench`, `time`, `estimate_timer_overhead`, the first-bench warm-up,
`--samples`/`--ms`/`--loops`/`--maxLoops`/`--overheadGoal`/`--match`/`--config`/`--outResultsFile`/`--dryRun`/`--quiet`,
`Stats` (`tools/Stats.h`), `NanoJSONResultsWriter` and the console table. It leaves out GPU
targets, `BenchmarkStream`'s SKP/SVG/image/GM sources, tracing and pprof. It adds:

- `--writeRaw <dir>`: raw canvas bytes and a JSON sidecar per (benchmark, config), in the format
  of `OracleDump`, for C3.
- `--cpuCap {baseline,ssse3,ml3,ml4}`: the counterpart of `SKIA_ORACLE_CPU_CAP`. It goes through
  `Tier::detect_with_cap` (`crates/skia-rust-simd/src/tier.rs#L126`) in the production dispatch
  path. **The perf build must not enable the `testing`, `models` or `emulated-estimates` features
  of `skia-rust-simd`.** The bench crate pulls `skia-rust-gm` only under its `check` feature, so
  `cargo build --release -p skia-rust-bench --bin nanobench` is a production build.
- `options.manifest_id` and `options.loops` in the JSON (free-form option strings in nanobench's
  schema), and a `key` block with `impl = "skia-rust"`, the CPU model, the estimate fingerprint
  (`cargo xtask cpu-probe`) and the tier.

`crates/skia-rust-simd/examples/rp_bench.rs` and `oracle/rp-bench/` were the A3 stopgap. They
retire when `SkRPBench` (B11) is ported.

---

## 5. Comparing with Skia without the server (Q1, Q4, Q5, Q6)

### 5.1 Options

| Option | How | Cost | Verdict |
|---|---|---|---|
| A. Stored Skia numbers | Publish Skia's timings once, compare later runs with them | Cheap | **Rejected.** Hosted runners vary in CPU model (AMD EPYC and Intel Xeon both appear), clock and neighbours from job to job, so stored absolute numbers are meaningless |
| B. **Same job, both binaries, interleaved** | Build Skia's nanobench at the pin in CI (cached per pin), run it and ours alternately in one VM | One cold Skia build per pin and per platform (about 45–60 min on 4 vCPUs, estimate), then a cache hit. Runs: §5.3 | **Recommended** |
| C. Link Skia into a Rust bench via FFI | One process times both | A C++ build plus FFI glue (rust-skia's build is no shortcut: it is a different Skia revision) | Rejected. Same build cost as B, more code, and FFI on the hot path skews small benches |
| D. Instruction counts (cachegrind, `perf stat`) | Deterministic counts instead of time | Cachegrind is about 50× slower. Hosted VMs may not expose hardware counters (B5 checks) | Not a parity measure (instruction mix differs between implementations). Kept as an optional, noise-free **ours-over-time** regression signal |
| E. Self-hosted bare-metal runner | PLAN's original setup | A machine | Q5 alternative. The tooling of B is the same, so moving later is just a runner label |

### 5.2 The CI Skia build (B3)

- **Where:** `ubuntu-latest` x64 first. `ubuntu-24.04-arm`, `macos-latest` and `windows-latest`
  (clang-cl, as the oracle used) follow as report-only builds.
- **What:** the pinned checkout (already cached by the `inventory` job), `oracle/patches/skia-oracle.patch`
  extended so that nanobench, like DM, gets `--oracleRawPath`, the `options.loops` field and
  `SKIA_ORACLE_CPU_CAP` (the cap already lives in `SkCpu`, so nanobench inherits it),
  `tools/git-sync-deps`, and `ninja nanobench`.
- **GN args:** the oracle's shared args (`is_debug=false`, `is_official_build=false`,
  `skia_enable_tools=true`, `skia_use_gl=false`; nanobench needs Ganesh compiled in, `BUILD.gn#L3182`),
  clang with Skia's default `-ffp-contract=off`, and `skia_use_partition_alloc=false`
  (`gn/skia.gni#L132` turns it on for standalone clang builds), so that both sides use the
  platform `malloc` (Q9).
- **Cache:** key = pin, GN args, patch hash and runner image version. The cached item is the
  `nanobench` binary (about 100 MB) plus `resources/`, not `third_party/externals`.
- **Validation of the build (acceptance test of B3):** the same job builds `dm` and renders 50 GMs
  at `SKIA_ORACLE_CPU_CAP=baseline` and `ml3`. They must match `goldens-m156`'s
  `cpu-x64-sse2-rgba` and `-rt-ml3` hashes. That shows Linux clang and glibc libm reproduce the
  Windows oracle on that set. Any mismatch is a finding written into `oracle/README.md`, and the
  bench goldens are then labelled as Linux-host tiers.
- **Outputs, each as a workflow artifact, published by the maintainer (Q10):**
  - `bench-names-m156.json`: for every runtime benchmark, its unique name, size and suitable
    configs (from `--dryRun` plus one smoke sample), and the `DEF_BENCH` call site. The call site
    comes from a two-line patch that records `__FILE__`/`__LINE__` in `BenchRegistry`. This
    file is C1's reference, and it maps Skia's result names to manifest ids.
  - Bench goldens: `nanobench --config 8888 [565 f16] --loops 1 --samples 1 --oracleRawPath …
    --nativeFonts false` at caps `baseline` and `ml3`, with the runner's estimate fingerprint
    recorded. The tiers are named `bench-x64-sse2-rgba` and `bench-x64-sse2-rgba-rt-ml3`. The
    rgba build is the Linux default, N32 = RGBA. Hashes go into `hashes-m156.json`'s format;
    objects are optional, because C3 needs hashes and diffs can be re-rendered.
- **Vendor estimates** (`raster-pipeline.md` §4.6): a bench golden made on an Intel runner differs
  from one made on AMD wherever `rcpps`/`rsqrtps` reach the output. The fingerprint stored with
  the goldens selects the emulated estimate tables on our side, exactly as for GMs. Without it,
  those benches are "not checkable" rather than failing.

### 5.3 Running a comparison (`cargo xtask bench vs-skia`, B4)

1. **Select** the benchmarks: everything (nightly, sharded), or the canary set plus the
   registrations in files the PR touched (PR runs).
2. **Calibrate once on Skia's side.** Run Skia's nanobench with auto-tuned loops and
   `--overheadGoal 0.00001` (samples about 10× longer than the default, a few ms; B5 tunes it),
   and read `options.loops`. **Both sides then run with that fixed `--loops`**, so each sample is
   the same work. That matters because our port could be slower and nanobench's auto-tuning
   would then give it fewer loops.
3. **Interleave.** For round r = 1..R (default 5): Skia on the batch, then ours, with the order
   alternating between rounds, each `--samples 10`. Batches are about 50 benchmarks, so drift
   within a round is short.
4. **Same conditions:** `taskset -c 1` on Linux (no pinning on macOS; `start /affinity` on
   Windows); `SKIA_ORACLE_CPU_CAP=ml3` and `--cpuCap ml3` on x64, native Neon on arm64; the same
   `--config`; `--nativeFonts false`; the same resources directory. The first sample of every
   (benchmark, round) is dropped on both sides (fixed loops disable nanobench's warm-up). Both
   processes print the CPU model and fingerprint, and the comparator rejects a round whose two
   halves report different ones.
5. **Merge** the per-round JSON into one sample set per (result key, config, side).

Cost, estimated: about 1,600 (benchmark, config) pairs in scope (`8888` + `nonrendering`) ×
2 sides × R = 5 rounds × 10 samples × about 5 ms ≈ 2.2 h of samples, plus setup. **Sharded 8
ways that is about 20–25 min per shard**, nightly. A PR run (canary set of about 150 plus touched
files) takes about 10 min. B5 replaces these estimates with measurements.

### 5.4 The statistic (`cargo xtask bench compare`, B4)

For each (benchmark, config), with Skia's samples S and ours O (ms per loop per unit):

- **Test:** one-sided Mann-Whitney U on O versus S·(1 + δ), H₁ "ours is slower than Skia by more
  than δ". This is PLAN §9.2 ("slower **and** significant") with a margin. With δ = 0 it is
  exactly PLAN's rule.
- **Multiplicity:** Benjamini-Hochberg at q = 0.05 over all comparisons of the run. Without it,
  PLAN's per-bench p < 0.05 on about 1,500 comparisons flags about 75 benchmarks even when the
  two binaries are identical.
- **Confirmation:** a benchmark flagged `slower` is re-run in a fresh job (another VM) with R = 10.
  It counts only if it is flagged again.
- **Effect size:** the Hodges-Lehmann estimate of the log ratio, with a 95% interval, is reported
  for every benchmark, as is nanobench's median (`sorted[n/2]`) for each side.
- **Entry verdict:** `parity` when no runtime benchmark of the entry is `slower` in any gated
  config. `slower` lists the offenders.
- **δ (Q4):** B5 measures it. A/A runs (Skia against Skia, and ours against ours, as two
  processes with the same interleaving) on each runner class give the false-positive rate as a
  function of δ. The proposal is the smallest δ, in steps of 0.5%, for which A/A sweeps report
  zero confirmed `slower` verdicts in 5 of 5 nights. 2–4% is expected. PLAN's δ = 0 stays the
  rule for a dedicated machine (option E).

### 5.5 Where results go (B6, Q11)

- **PR runs:** a markdown table (slower first, then the biggest wins) posted as a PR comment
  through the REST API, plus the raw JSON as an artifact. The check is advisory until B5 is done
  and required for PRs labelled `perf` afterwards (Q6).
- **Nightly:** the merged JSON and the verdicts are pushed to a `perf-data` branch
  (`<date>/<runner-class>.json`), which feeds the dashboard's time series (PLAN §9.2, "results
  are kept as a time series"). A new confirmed `slower` verdict opens an issue that names the
  entry, the ratio and the commit range.
- **Ledger:** `cargo xtask bench ledger` reads the latest nightly data and prints the per-entry
  verdict table (§3.2). It is generated, not committed, so the manifest stays the only committed
  status file.

---

## 6. CI shape

| Job | Trigger | Runs | Gating |
|---|---|---|---|
| `test` / `test-release` (existing) | every PR | bench smoke `#[test]`s (C2) in debug and release on 4 platforms | yes (as today) |
| `inventory` (existing) | every PR | `inventory verify` incl. `bench-verify` (C1–C3 on Linux x64) | yes |
| `wasm`, `miri` (existing) | every PR | bench crate excluded | — |
| `skia-nanobench` (new, B3) | pin change, manual | build and cache Skia's nanobench and dm; names list; bench goldens | — |
| `bench-perf` (new, B6) | nightly (8 shards), PR label `perf` | §5.3–5.5 | advisory, then labelled PRs (Q6) |

The smoke tests add one `#[test]` per registration. Each runs `draw(1)` once per suitable
config. A few benches are heavy even at one loop (`GrResourceCache`-style setups are excluded;
`DecodeBench`, `PDFBench` and `SkGlyphCacheStressTest` remain). In debug, their setup sizes come
from Skia's own constants and are not reduced (no weakening). If the debug job's time grows by
more than a few minutes, the fix is to run `bench-verify` in release only, as is done for
`gm-verify`, never to skip a bench.

---

## 7. Work breakdown

Sizes: S (≤ 1 day of agent work), M, L. **(S)**/**(O)** marks tasks that need Sonnet or Opus.
Unmarked tasks are mechanical ports for the default rung. "Unlocks" counts manifest entries
(after the B0 relabel). Sweep tasks can be split by file across agents; each file is one PR.

### Wave B-0: bookkeeping, harness, verification (start now)

| ID | Task | Sources | Depends | Size | Unlocks |
|---|---|---|---|---|---|
| B0 | Manifest: relabel modules (§1.2), exclude the 111 Ganesh entries with a reason (Q7), apply Q8; `policy_exclusion` learns `#include "src/gpu/ganesh/` and `kGanesh`-only suitability so `inventory sync` keeps them excluded; PLAN §7 (server-only checks), §8.2 (the "server-check status") and §9.2 edits from this note | `xtask/src/inventory.rs`, `docs/PLAN.md` | — | S | 111 excluded |
| B1 **(S)** | `skia-rust-bench` crate: `Benchmark` trait, `Backend`, `def_bench!`/`def_bench_set!`, registry, smoke `#[test]` generation; nanobench CPU-path port (§4) with `--writeRaw`, `--cpuCap`, JSON; `skia-rust-simd` production cap entry point if `detect_with_cap` needs one | `bench/Benchmark.{h,cpp}`, `bench/nanobench.cpp` (CPU parts), `bench/ResultsWriter.h`, `tools/Stats.h` [~1.2k] | — | L | — (enables all) |
| B2 | `bench-verify` binary, `xtask/src/verify_benches.rs`, `inventory verify` third leg, `module-path` for bench ids (prints the name literal), PORTING.md §13 "Porting benches" | `xtask/src/{verify,verify_gms,inventory}.rs` | B1 | M | — |

### Wave B-A: sweeps of already-ported code (parallel after B1 + B2; C1 by review, C2 now, C3 when B3 lands)

| ID | Files | Depends (feature) | Size | Unlocks |
|---|---|---|---|---|
| B10a | Geometry and math: `MathBench` 13, `Matrix44Bench` 16, `MatrixBench` 24, `GeometryBench` 11, `CubicMapBench` 10, `InterpBench` 5, `PolyUtilsBench` 25, `RegionBench` 18, `RegionContainBench` 1, `RTreeBench` 8, `PathIterBench` 5, `FindCubicConvex180ChopsBench` 2, `QuickRejectBench` 2, `BezierBench` 6 | Phase 1 (done) | L | 146 |
| B10b | Memory, utils, colour: `MemsetBench` 27, `SortBench` 20, `SwizzleBench` 10, `Sk4fBench` 5, `ColorPrivBench` 4, `PremulAndUnpremulAlphaOpsBench` 2, `ChecksumBench` 2, `StreamBench` 2, `WriterBench` 1, `TableBench` 1, `JSONBench` 3, `ControlBench` 1, `RefCntBench` 6 + `MutexBench` 4 (Q8) | Phase 1; `SkJSON` for `JSONBench` | M | 88 |
| B11 | Raster pipeline, blending, pixels: `SkRPBench` 7, `BlendmodeBench` 29, `RotatedRectBench` 5, `AlternatingColorPatternBench` 9, `ClearBench` 3, `FSRectBench` 1, `CoverageBench` 2, `ReadPixBench` 13, `WritePixelsBench` 8, `ColorSpaceBench` 5 | RP, blitters, skcms (done) | M | 82 |
| B12a | `PathBench` 87 | Canvas D6 (done) | L | 87 |
| B12b | Shapes, strokes, clips: `RectBench` 37, `ShapesBench` 15, `HairlinePathBench` 14, `StrokeBench` 12, `AAClipBench` 11, `BigPathBench` 6, `LineBench` 5, `ChartBench` 2, `GameBench` 15, `BulkRectBench` 2, `CanvasSaveRestoreBench` 4, `ClipStrategyBench` 8 | D6 | L | 131 |
| B12c | Vertices, patches, shadows: `VertBench` 13, `PatchBench` 50, `ShadowBench` 4 | `drawVertices`/`drawPatch`, `SkShadowUtils` (if ported) | M | 67 |
| B13 | Images and sampling: `DrawBitmapAABench` 8, `BilerpBench` 5, `MipmapBench` 10, `TileBench` 6, `RepeatTileBench` 3, `FilteringBench` 6, `ImageCacheBench` 1 | image drawing, mipmaps | M | 39 |
| B14 | Pictures: `PictureNestingBench` 18, `PicturePlaybackBench` 4, `PictureOverheadBench` 1 | D7 pictures | S | 23 |
| B15 | `DashBench` 29 | C7 dash (done) | M | 29 |
| B16 | Gradients and noise: `GradientBench` 42, `HardStopGradientBench_*` 48, `PerlinNoiseBench` 1 | gradients (done), Perlin shader | M | 91 |
| B17 | Blur and mask filters: `BlurBench` 21, `BlurRectBench` 36, `BlurRectsBench` 2, `ShaderMaskFilterBench` 4, `ClipMaskBench` 1 | blur mask filter, `SkBlurMask` | M | 64 |
| B18 | Image filters: `BlurImageFilterBench` 36, `ImageFilterDAGBench` 4, `ImageFilterCollapse` 2, `TileImageFilterBench` 3, `LightingBench` 12, `MorphologyBench` 7, `DisplacementBench` 6, `MatrixConvolutionBench` 10, `MergeBench` 2 | image filters parts 1–3 (merged); SkSL-backed filters wait for B20 | L | 82 |
| B19 | `ColorFilterBench` (non-runtime) 17 | colour filters | S | 17 |

Wave B-A total: **946** entries, of which about 350 are non-rendering and can reach `passing`
before B3. The rest reach `ported`.

### Wave B-B: Skia in CI and the comparison (needs Q1)

| ID | Task | Depends | Size | Unlocks |
|---|---|---|---|---|
| B3 **(S)** | `skia-nanobench` workflow (§5.2): oracle patch extension (nanobench `--oracleRawPath`, `options.loops`, registry call sites), cached Linux x64 build, `dm` cross-check against `goldens-m156`, `bench-names-m156.json`, bench goldens at `baseline` and `ml3` with fingerprint; `xtask oracle` learns the `bench-*` tiers | Q1, Q10 | L | C1 and C3 data: rendering entries move from `ported` to `passing` (about 600 of B-A's) |
| B4 **(S)** | `xtask bench {run, vs-skia, compare, ledger}`: calibration, interleaved rounds, pinning, caps, JSON merge, Mann-Whitney U with δ, BH-FDR, Hodges-Lehmann, confirmation list, markdown report | B1, B3 | M | perf verdicts |
| B5 **(O)** | Calibration: A/A and A/B sweeps on each runner class, the false-positive rate as a function of δ, sample length (`--overheadGoal`), R, shard size; check hardware counter availability (option D); write the numbers into this note and propose final δ, R and gating platforms | B4 | M | Q4, Q5 answered with data |
| B6 | `bench-perf` workflow: nightly shards, `perf` label trigger, canary set, PR comment, `perf-data` branch, issue on new confirmed `slower` | B4, B5 for gating | M | the gate |
| B7 | Report-only platforms: Skia nanobench on `ubuntu-24.04-arm` (Neon; with DM, a first Neon GM golden set as a by-product, `raster-pipeline.md` §4.5), `macos-latest`, `windows-latest` (clang-cl) | B3 | M | report |

### Wave B-C: benches that wait on other phases

| ID | Files | Depends | Size | Unlocks |
|---|---|---|---|---|
| B20 | `SkSLBench` 9 (in-scope outputs: SkSL, SkRP, WGSL; GLSL/Metal/SPIR-V omitted), `ColorFilterBench` runtime 3, the SkSL-backed image filter benches | `sksl.md` S30, G19 for `graphite_*` | M | 12 |
| B21 | Text: `CmapBench` 8, `TypefaceBench` 7, `SkGlyphCacheBench` 5, `TextBlobBench` 3, `FontCacheBench` 3, `PathTextBench` 3, `GlyphRunRSXformBench` 1 | `text.md` T26 (slices 1–12 merged) | M | 30 |
| B22 | Codecs: `EncodeBench` 28, `DecodeBench` (bitmap) 6, `WebpBlendBench` 1 | `codecs.md` C21 | M | 35 |
| B23 | `PathOpsBench` 8 | PathOps (done) | S | 8 |
| B24 | GPU-shared CPU code: `RectanizerBench` 6, `TessellateBench` (shared) 6, `BoundsManagerBench` 6, `IntersectionTreeBench` 6, sparse strips `FlattenBench` 6, `TilerBench` 4, `CoverageBench` 6 | `gpu.md` G1, G2, G4, G17 | M | 40 |
| B25 | `PDFBench` 12 | Phase 7 PDF | M | 12 |
| B26 | Skottie: `Skottie{,Picture}DecodeBench` 50 | Phase 7 Skottie | M | 50 |
| B27 | `ShaperBench` 34, `ParagraphBench` 2 | Phase 7 shaper, paragraph | M | 36 |

Wave B-C total: **223**. Waves B-A and B-C together: **1,169** = 1,280 − 111 excluded.

### Dependency summary

```
B0 ─┐
B1 ─┴─ B2 ─┬─ B10a … B19 (parallel; C1 by review + C2 now) ─────────┐
           │                                                        ├─ rendering entries → passing
Q1 ─ B3 ───┴─ (names list, bench goldens) ──────────────────────────┘
B1 + B3 ─ B4 ─ B5 ─ B6 (gate)        B3 ─ B7 (report-only platforms)
other phases ─ B20 … B27
```

Critical path to the first perf verdict: B1 → B3 → B4 (about 3 PRs). Critical path to the gate:
B4 → B5 → B6. The sweeps (B10–B19) need only B1 and B2 and can run in parallel with B-B.

---

## 8. Risks and open questions

| # | Item | Plan |
|---|---|---|
| Q1 | **Build Skia's nanobench (and `dm` for validation) at the pin in CI** on hosted Linux x64 (later arm64, macOS, Windows). One cold build per pin per platform (about 45–60 min, estimate), cached; nightly comparison minutes (§5.3) | Recommended. Without it there are no C3 goldens and no perf comparison; rendering benches stay `ported` and perf stays unmeasured |
| Q2 | **What `passing` means for benches** | Recommended: correctness only (C1–C3); perf is a separate ledger (§3.2). Alternative: `passing` requires perf parity, which would leave nearly all entries `todo` while the interpreter is slower |
| Q3 | **Rendering benches before or without bench goldens** | Recommended: `ported` until C3 data exists. Alternative: `passing` on C1 and C2. Also decide whether C3 covers `565`/`f16` (recommended: `8888` only; the GM suite already covers the others) |
| Q4 | **The noise margin δ on hosted runners**, with BH-FDR and a confirmation re-run | Recommended: δ measured by B5 (2–4% expected); PLAN's δ = 0 kept for a dedicated machine |
| Q5 | **Gating platforms and tier** | Recommended: `ubuntu-latest` x64 at `ml3`, configs `8888` + `nonrendering`; arm64 Linux, macOS and Windows report-only. Alternative: a self-hosted bare-metal runner (option E), same tooling |
| Q6 | **When the perf check runs and whether it blocks** | Recommended: nightly full sweep plus PR label `perf`; advisory until B5, then required for labelled PRs; a new confirmed `slower` verdict nightly opens an issue. PLAN §8.2 "server-check status" is replaced by this |
| Q7 | **Exclude the 111 Ganesh-internal benches** | Recommended (same reason as the excluded Ganesh unit tests) |
| Q8 | **`RefCntBench`/`MutexBench` (10)** time types Rust replaces | Recommended: port them as benches of `Arc`/`Weak`/`Mutex`/`RwLock`, report-only, with verdict `not-comparable` for the gate. Alternative: exclude |
| Q9 | **Build parity**: Skia with PartitionAlloc off (both sides on the platform `malloc`); GN's default `-O3`, no LTO; ours in a `bench` profile = release with `codegen-units = 1`, no LTO | Recommended as stated. Alternatives: compare against Skia as Chrome ships it (PartitionAlloc on), or allow thin LTO on our side |
| Q10 | **Where the bench goldens and the names list are published**: new assets in `goldens-m156` (one lock file, `xtask oracle publish`) or a separate `bench-goldens-m156` release; CI produces artifacts and the maintainer publishes | Recommended: new assets in `goldens-m156`, published by the maintainer from the workflow's artifacts (no release-write token in CI) |
| Q11 | **Where perf results live** | Recommended: a `perf-data` branch (time series) plus the dashboard; nothing perf-related committed to `main` |
| Q12 | **Benchmarks outside the manifest** (nanobench's GM, SKP, SVG and image streams) | Out of scope for now. GM-as-bench is a cheap later addition (every passing GM becomes a drawing benchmark, with no porting needed) |
| R1 | **Shared-runner noise** (neighbours, frequency changes, CPU model mix) | Same-VM interleaving, a pinned core, fixed loops, longer samples, BH-FDR, a confirmation re-run, δ from A/A data (B5) |
| R2 | **The CI Skia build differs from the oracle's** (Linux clang and glibc libm against Windows clang-cl and UCRT) | B3's `dm` cross-check against `goldens-m156`; bench goldens are labelled as Linux tiers; any difference is recorded in `oracle/README.md` |
| R3 | **Vendor-dependent estimates in bench goldens** | Fingerprint stored with the goldens; emulated estimate tables on our side (`raster-pipeline.md` §4.6); otherwise "not checkable", never failing |
| R4 | **The perf binary built with test features** (`testing`, `models`, `emulated-estimates`) would time the wrong code | `check` feature split (§4); B4 refuses a binary whose `key` reports test features |
| R5 | **Ports that do different work** (hoisted setup, missing `black_box`, different `SkRandom` use) | §2.4 rules, C3 for rendering benches, reviewer checklist in PORTING.md §13 |
| R6 | **Sweep duration** grows with the ported count | 8 shards; only `8888` + `nonrendering` gated; PR runs use the canary set; shard count is a workflow input |
| R7 | **Cache eviction** (10 GB per repository; the GM goldens and Skia checkout are also cached) | The cached Skia item is the binary (about 100 MB), not the externals; a miss costs one rebuild |
| R8 | **Many early `slower` verdicts** (the interpreter, issue #36) | Expected, and a reason the verdict is not a manifest status. The ledger guides optimization work, which has to keep every exactness test green |

No code is included. B1 creates the crate together with its first registrations (B10a's
`MathBench` is a good pilot: non-rendering, small, Phase 1 code only).

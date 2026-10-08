# Design: SkSL (Phase 3 CPU, with Phase 6 WGSL in mind)

Status: proposed (2026-10-08). Pin: `chrome/m156` (`95ee33d7ec77`). Audience: every agent porting
Skia's SkSL compiler (`src/sksl`), its Raster Pipeline back end, `SkRuntimeEffect` and the effects
built on it, and later the WGSL and pipeline-stage code generators.

All Skia references are `path#Lx-Ly` in the pinned tree (`third_party/skia`). "RP" = Raster
Pipeline. Manifest counts are from `inventory/manifest.toml` at `88efa63`. Counts that mention the
image-filter work come from `origin/port/image-filters-3` (`1ad28be`), which marks 39 entries as
blocked on SkSL. GM counts are marked *file scan* when they come from grepping the GM source file
for an API, not from running the GM; read those as upper bounds.

## Decisions at a glance

1. **The SkSL crate sits below core.** The layering becomes `simd → base → sksl → core → …`.
   `skia-rust-sksl` holds the compiler: lexer, parser, IR, analysis, optimizer, the RP builder and
   code generator, and later the WGSL and pipeline-stage generators. It depends on
   `skia-rust-simd` (for `rp::Op`, `Stage` and the contexts) and on a new `skia-rust-base` crate.
   **`RuntimeEffect`, `RuntimeShader`, `RuntimeColorFilter`, `RuntimeBlender` and
   `KnownRuntimeEffects` live in `skia-rust-core`**, where Skia has them (`src/core`, `src/shaders`,
   `src/effects/colorfilters`). Every Skia caller of a runtime effect then sits above `sksl` with
   no seams and no registries: `FilterResult`'s decal (core), the shader blur algorithm (raster),
   `SkShaders::Blend` with a blender (core), and the image filters (effects). This revises PLAN
   §3.1/§3.2 (§2).
2. **A small `skia-rust-base` crate** receives core's self-contained math modules that SkSL needs:
   `math`, `floating_point`, `scalar`, `t_fits_in`, `to`, `safe_math`, `matrix_invert`,
   `checksum`, `half` (about 1.9k lines, all safe, no dependencies outside simd). Core re-exports
   them at their current paths, so no caller changes (§2.2).
3. **Built-in modules are compiled from SkSL text at run time, as Skia does.** m156 has no binary
   module dump: `SkSLModuleDataDefault.cpp` `#include`s the minified *SkSL source* of each module
   and the compiler parses it on first use. We embed both variants Skia uses: the **minified**
   texts (`src/sksl/generated/*.minified.sksl`, which the release oracle used for every GM and unit
   test) and the **original** sources (`src/sksl/sksl_*.sksl`, which `skslc` read when it generated
   every `tests/sksl` golden). The two variants give observably different output. The `.wgsl`
   goldens contain the original parameter names (`s`, `d`, `_0_result`), not the minified ones
   (`a`, `b`, `$b`). A test checks that the embedded texts are identical to the pinned tree, and
   S24's ported `sksl-minify` regenerates the minified texts from the originals (§3).
4. **IR: arenas with typed ids, in layered pools, like PathOps.** Every symbol, expression,
   statement and program element lives in a pool and is named by a `Copy` id (`TypeId`, `VarId`,
   `FnId`, `ExprId`, `StmtId`, `SymTabId`, …). Skia's `std::unique_ptr<Expression>&` slot becomes
   an `ExprId` whose node is overwritten in place. Built-in modules are frozen pools shared through
   `Arc`. A program's pool extends its module chain with an id offset, so pointer identity
   (`type == *fContext.fTypes.fFloat`) becomes id equality, and built-in type ids are constants.
   We rejected owned trees: SkSL is a graph (references to variables and declarations,
   declaration↔definition back-pointers, shared module symbols), and the inliner keeps pointers to
   statement slots across passes (§4).
5. **Byte-exact outputs.** The `.skrp` dump, the error text (`### Compilation failed:` + `error:
   <line>: <msg>` + echoed line + carets + `N errors`), minified SkSL, `.stage` and WGSL must all
   match byte for byte. This pins down: the optimizer pass order and repeat loops, the inliner's
   candidate order and mangler counters, the RP builder's peephole rewrites, `%g`-style float
   printing (`skstd::to_string`), and **iteration order of one Skia hash set**
   (`findPreexistingImmutableData` iterates a `THashSet<Slot>`). The last one needs a faithful
   `SkTHashTable` port, not `std::HashSet` (§5).
6. **Two build flavours of the compiler, chosen by an explicit value, never by a global.**
   `Flavor::Library` behaves like Skia built as a library (minified modules, no
   `appendStackRewindForNonTailcallers` rewinds; Skia leaves them out when `SK_HAS_MUSTTAIL`).
   `Flavor::Standalone` behaves like `skslc` (`SKSL_STANDALONE`: original modules, stack rewinds in
   `.skrp` dumps). Skia's static `Compiler::sOptimizer`/`sInliner` override flags become settings
   (§5, §6).
7. **SkSL slot memory is shader scratch memory.** `Program::appendStages` allocates the
   values/stack/immutable slab with the existing `ArenaAlloc::alloc_scratch` (bound to
   `SHADER_SCRATCH` by whoever runs the pipeline). It gains an initial-contents variant, because
   Skia writes the immutable slots when it builds the stages. Byte offsets use the lane count of
   `skia_rust_simd::selection()` at append time, the equivalent of `SkOpts::raster_pipeline_highp_stride`.
   A debug assertion checks that the pipeline compiles on the same selection (§6).
8. **The `.skrp` dump is the RP oracle.** The SkSL stages already pass on every tier (B6a–d, rp-diff),
   so the pixels of a runtime effect are right once its stage list is right. The 365 `.skrp` goldens
   and the 26 `RasterPipelineBuilderTest` dumps check the stage list exactly, with no C++ oracle
   run. GMs then only check the integration (§6, §9).
9. **First unlocks.** The RP *builder* depends on nothing in the front end and turns the 26
   `RasterPipelineBuilderTest` entries green first. The front end alone turns **114** entries green
   (front-end-only goldens and unit tests), or **452** if the 338 `errors/*.glsl` error-text goldens
   are reclassified as in scope (they never reach the GLSL generator; S0). The RP code generator
   then unlocks 280 `.skrp` + 69 `SkSLTest` + 9 more. `RuntimeEffect` unlocks the remaining 183
   `SkSLTest` entries, `SkRuntimeEffectTest` and the first GMs (§10).
10. **Port the known runtime effects in this order.** First the color filters and blenders that
    need no image-filter machinery: Luma, HighContrast, Overdraw, Lerp, Arithmetic, Blend (10
    entries named in manifests, 3 more once the arithmetic image filter exists, and about 10 by
    file scan). Then the 1D/2D blur and Decal effects.
    These are the biggest lever: on CPU the f16 config blurs through `SkShaderBlurAlgorithm`,
    so every blur image filter GM needs SkSL for its f16 result (up to ~108 GM entries by file
    scan). Last, morphology, matrix convolution, displacement, lighting/normal and magnifier,
    together with their filter ports (26 named entries, including the 3 arithmetic-filter ones) (§8).
11. **WGSL is CPU-testable and can start in Phase 3.** The 420 `.wgsl` goldens are pure compiler
    output and need no GPU. The IR, the modules and the parser are built for every program kind
    from the start (compute, vertex, mesh, interface blocks), so the WGSL generator (Phase 6
    scope) can run as a parallel wave once the optimizer lands (§9).

---

## 1. What exists and what the tests compare

### 1.1 Skia's SkSL at m156

| Part | Files | C++ lines |
|---|---|---|
| Lexer (generated DFA) + token/position/operator/strings | `SkSLLexer.{h,cpp}` (generated from `lex/sksl.lex`), `SkSLPosition`, `SkSLOperator`, `SkSLString` | ~2.6k (2.1k tables) |
| Parser | `SkSLParser.{h,cpp}` | 2.7k |
| IR (nodes, their `Convert`/`Make`, `description()`, `clone()`) | `ir/*` | 15.4k incl. headers (9.3k `.cpp`; `SkSLType` 1.4k, `SkSLFunctionCall` 1.3k) |
| Compiler, context, modules, builtin types, constant folder, inliner, utils | `SkSLCompiler`, `SkSLContext`, `SkSLModuleLoader`, `SkSLBuiltinTypes`, `SkSLConstantFolder`, `SkSLInliner`, `SkSLMangler`, `SkSLUtil` (`ShaderCaps`), `SkSLMemoryLayout.h` | ~4.5k |
| Analysis | `SkSLAnalysis.cpp`, `analysis/*` | 3.6k |
| Transforms (optimizer passes, finalization) | `transform/*` | 2.1k |
| RP back end | `codegen/SkSLRasterPipelineBuilder.{h,cpp}`, `SkSLRasterPipelineCodeGenerator.cpp` | 8.7k |
| Tracing | `tracing/*` | 0.8k |
| Pipeline-stage generator (Graphite runtime effects; `.stage`) | `codegen/SkSLPipelineStageCodeGenerator.cpp` | 0.9k |
| WGSL generator (Phase 6) | `codegen/SkSLWGSLCodeGenerator.cpp` (+ Tint validator) | 5.1k |
| Not ported: GLSL, Metal, SPIR-V, HLSL generators | `codegen/SkSL{GLSL,Metal,SPIRV,HLSL}*` | 11.8k |
| Runtime effects (core) | `SkRuntimeEffect.cpp`, `SkRuntimeEffectPriv.h`, `SkRuntimeShader.cpp`, `SkRuntimeBlender.cpp`, `SkRuntimeColorFilter.cpp`, `SkKnownRuntimeEffects.cpp`, `include/effects/SkRuntimeEffect.h` | 2.9k |

In Rust that is about 45k lines for Phase 3 (front end + RP + runtime effects), plus about 6k for
pipeline-stage and WGSL.

**Already on `main`:** every SkSL raster-pipeline stage (B6a–d: lane masks, branches, copies,
swizzles, shuffles, n-way arithmetic, casts, transcendental functions, `inverse_mat*`, trace ops,
`callback`; only `perlin_noise` is still a stub). Their contexts are typed (`rp::contexts`). Slots
are addressed by `RpOffset` byte offsets from `set_base_pointer`'s `MemPtr`, so the lane count `N`
is baked into the offsets. Writable memory is bound per run through `MemoryBindings`.
`rp::contexts::TraceHook` was declared in simd as a stand-in for `SkSL::TraceHook` until an SkSL
crate exists. `ArenaAlloc::alloc_scratch` + `SHADER_SCRATCH` already give shaders writable
per-run memory. Today `blend_shader.rs` uses it.

### 1.2 The tests

**`sksl-golden` entries** (`tests/sksl/**`, generated by `skslc` from `resources/sksl/**`; 924 in
scope, 1,869 excluded as outputs of the GLSL/Metal/SPIR-V/HLSL generators):

| Output | In scope | How `skslc` produces it | Of which `### Compilation failed` | Needs |
|---|---|---|---|---|
| `.skrp` | 365 (folding 25, intrinsics 102, runtime 46, shared 186, realistic 5, skrp 1) | `Compiler::convertProgram` (`.sksl`/`.frag` inputs become `kPrivateRuntimeShader`), `MakeRasterPipelineProgram`, `RP::Program::dump(…, writeInstructionCount=true)` (`tools/skslc/Main.cpp#L707-L728`) | 106: 85 front-end errors, 21 `code is not supported` from the RP generator | front end; RP builder + dumper + code generator |
| `.wgsl` | 420 | `ToWGSL(…, PrettyPrint::kYes, IncludeSyntheticCode::kYes, ValidateWGSL)` | 7: 5 front-end errors, 2 `Tint compilation failed.` | front end; WGSL generator |
| `.minified.sksl` | 77 | `tools/sksl-minify`: compile, `optimizeModuleBeforeMinifying(shrinkSymbols)`, print each element's `description()`, strip whitespace with the lexer | 0 | front end + optimizer + minifier tool |
| `.stage` | 62 | `PipelineStage::ConvertProgram` with test callbacks, then `SkShaderUtils::PrettyPrint` (`Main.cpp#L729-L790`) | 7 (mesh errors) | front end; pipeline-stage generator |

Error goldens print the compiler's error text exactly as `Compiler::handleError`
(`SkSLCompiler.cpp#L441-L519`) formats it: line number, the source line (cut at 100 characters with
`...`), a caret run and a final `N error(s)` line. **The 338 `errors/*.glsl` goldens are
error-text-only too**: all 338 start with `### Compilation failed:`, because the error tests are
compiled only with `--glsl` (`gn/sksl_tests.gni#L1059-L1061`) and fail before any code generator
runs. The manifest excludes them as GLSL output. S0 reclassifies them (§1.4).

**Unit tests** (module `sksl` unless noted):

| File | Entries | Needs |
|---|---|---|
| `SkSLTest.cpp` | 252 (one per `SKSL_TEST`; each covers its `_CPU`, `_RP` and `_Clone` `DEF_TEST`s, `#L1026-L1031`) | `_Clone`: front end (`clone()` + `description()`). `_RP`: RP generator, run directly (`#L847-L990`). `_CPU`: `RuntimeEffect::make_for_shader` drawn on an N32 raster surface, optimized and `forceUnoptimized` (`#L588-L695`). 183 are `CPU`-flagged; for the 10 `GPU_ES3`-only ones `_RP` must *fail* (`report_rp_pass` errors with "NEW") |
| `SkRuntimeEffectTest.cpp` | 35 CPU (+2 Graphite in `gpu`, 8 excluded) | runtime effects; 3 also need tracing |
| `RasterPipelineBuilderTest.cpp` | 26 (filed as 23 `core` + 3 `effects`) | RP `Builder` + `Program::dump` only: no front end |
| `RasterPipelineCodeGeneratorTest.cpp` | 9 | front end + RP generator, run |
| `SkSLDebugTracePlayerTest.cpp` / `SkSLDebugTraceTest.cpp` | 15 / 5 | tracing (+ RP generator for the player); `SkSLTraceUtils` JSON write/read (`tools/sksltrace`) for `DebugTracePriv` |
| `SkSLErrorTest.cpp` | 8 | front end (substring matching of `/*%%* … *%%*/` expectations) |
| `SkSLMemoryLayoutTest.cpp` | 8 | `MemoryLayout` + builtin types |
| `SkSLTypeTest.cpp` | 1 | types |
| `SkSLES2ConformanceTest.cpp` | 2 | runtime effects |
| `SkSL{GLSL,Metal,SPIRV}Testbed.cpp` | 3 | generators not ported → exclude (S0) |
| `SkSL{WGSL,PipelineStage}Testbed.cpp` | 2 | WGSL / pipeline-stage generator |
| `RuntimeBlendTest.cpp` (`effects`) | 1 CPU | runtime blender |
| `HighContrastFilterTest`, `ImageFilterTest` (13), `ShaderImageFilterTest` | 16 | known runtime effects + image filters |

**GMs.** These GMs run a runtime effect directly: `runtimeshader` 17, `runtimeintrinsics` 6,
`runtimecolorfilter` 2, `runtimeimagefilter` 2, `runtimefunctions` 1, `composecolorfilter` 2,
`workingspace` 2, `rippleshadergm`, `kawase_blur_rt`, `imagedither`, `destcolor` (about 36). GMs
that use a known effect are covered in §8. `mesh.cpp` (11) needs `SkMeshSpecification` to compile
mesh SkSL; on CPU `SkBitmapDevice::drawMesh` is a no-op (`SkBitmapDevice.cpp#L559-L561`). Benches:
`SkSLBench.cpp` (7 + 2 GPU), `ColorFilterBench` (2). Skottie: 3 `skottie-sksl-*` files.

### 1.3 How the expected results were produced

| | `tests/sksl` goldens | DM goldens and unit tests (the oracle) |
|---|---|---|
| Binary | `skslc`, `SKSL_STANDALONE` + `SK_DISABLE_TRACING` (`BUILD.gn#L719-L760`) | DM, release (`is_debug=false`) |
| Module text | `SkSLModuleDataFile.cpp`: the original `src/sksl/sksl_*.sksl`, copied next to `skslc` (`BUILD.gn#L568-L588`) | `SkSLModuleDataDefault.cpp`: `generated/*.minified.sksl` (release ⇒ `!SK_DEBUG`, `#L14-L30`) |
| Settings | `ProgramSettings` from `/*#pragma settings …*/` in the input (105 inputs; `Main.cpp#L349-L490`), `ShaderCapsFactory::Standalone()` (or the caps the pragma names), `fRTFlipOffset = 16384` | `SkRuntimeEffect::MakeSettings(options)` |
| RP stack rewinds | present (`appendStackRewindForNonTailcallers`, `SkSLRasterPipelineBuilder.cpp#L1631-L1637`) | absent when `SK_HAS_MUSTTAIL` (no effect on pixels; our interpreter treats rewinds as no-ops) |
| Program kind | by extension: `.rts` runtime shader, `.rtcf` color filter, `.rtb` blender, `.privrts` private, `.sksl`/`.frag` fragment (→ `kPrivateRuntimeShader` for `.skrp`), `.vert`, `.compute`, `.mvert`, `.mfrag` | per factory (`MakeForShader` …) |

### 1.4 Manifest corrections (task S0)

- `errors/*.glsl` (338): set `status = "todo"` and clear `reason` (hand-editable fields), and say
  why in the PR. `inventory sync` keeps the status of existing entries, but its default for new
  `.glsl` files stays `excluded`, so `xtask` (`scan_sksl_goldens`) also learns to treat
  `tests/sksl/errors/*.glsl` as in scope. Run them through the same harness as the `.skrp`
  failures. If a particular golden turns out to depend on GLSL caps
  beyond `Standalone()`, it goes back to `excluded` with that reason.
- `SkSL{GLSL,Metal,SPIRV}Testbed`: `excluded`, "exercises the GLSL/Metal/SPIR-V code generator,
  which skia-rust does not port".
- `RasterPipelineBuilderTest` (26): `module = "sksl"`.
- The 39 SkSL-blocked entries on the image-filter branch keep their reason. They flip with S20–S22.

## 2. Crates and layering

### 2.1 Placement (revises PLAN §3.1/§3.2)

```
skia-rust-simd ─ skia-rust-base ─ skia-rust-sksl ─ skia-rust-core ─ skcms/raster ─ effects ─ … ─ gpu
                                   (compiler, RP builder/codegen,      (RuntimeEffect, RuntimeShader,
                                    pipeline-stage, WGSL [feature])      KnownRuntimeEffects, …)
```

| Skia | skia-rust |
|---|---|
| `src/sksl/**` (all but the four excluded generators) | `skia_rust_sksl::{lexer, parser, ir, analysis, transform, compiler, module_loader, …}` |
| `src/sksl/codegen/SkSLRasterPipeline{Builder,CodeGenerator}` | `skia_rust_sksl::codegen::rp::{builder, program, dumper, generator}` |
| `src/sksl/tracing/*` | `skia_rust_sksl::tracing` (implements `skia_rust_simd::rp::TraceHook`) |
| `src/sksl/codegen/SkSLWGSLCodeGenerator`, `SkSLPipelineStageCodeGenerator` | `skia_rust_sksl::codegen::{wgsl, pipeline_stage}` (`wgsl` behind a crate feature that `gpu` turns on) |
| `src/core/SkSLTypeShared.h`, `include/private/SkSLSampleUsage.h`, `SkSLDefines.h` | `skia_rust_sksl` (SkSL-owned; core and gpu import them) |
| `src/core/SkRuntimeEffect.cpp`, `SkRuntimeEffectPriv.h`, `include/effects/SkRuntimeEffect.h` | `skia_rust_core::runtime_effect` (`RuntimeEffect`, `runtime_effect::{Uniform, Child, ChildPtr, ChildType, Options, RuntimeShaderBuilder, …}`, as skia-safe's `effects::runtime_effect`) + `runtime_effect_priv` |
| `src/shaders/SkRuntimeShader.cpp` | `skia_rust_core::shaders::runtime_shader` |
| `src/core/SkRuntimeBlender.cpp` | `skia_rust_core::runtime_blender` |
| `src/effects/colorfilters/SkRuntimeColorFilter.cpp` (+ Luma, Overdraw, Lerp factories) | `skia_rust_core::color_filters::runtime_color_filter` (core already hosts Skia's colorfilters) |
| `src/core/SkKnownRuntimeEffects.cpp` | `skia_rust_core::known_runtime_effects` |
| `src/effects/SkHighContrastFilter.cpp`, `SkBlenders.cpp`, image filters | `skia-rust-effects` (unchanged) |
| `tools/skslc/Main.cpp`, `tools/sksl-minify/SkSLMinify.cpp` | `tests/src/tools/{skslc, sksl_minify}.rs` (test tooling only) |

Why below core and not above it: Skia compiles SkSL into the same library as core, and core calls
runtime effects from inside its own machinery. Examples are `FilterResult`'s decal tiling
(`SkImageFilterTypes.cpp#L1438-L1452`), `SkShaders::Blend` with a non-mode blender
(`SkBlendShader.cpp#L148-L156`), the raster blur engine for non-8888 layers
(`SkBlurEngine.cpp#L1274-L1314`, in our raster crate) and flattenable registration. With SkSL
above core, each of these would need a seam or a global registry. With SkSL below core, the
compiler needs only a dozen helpers from core (`SkSafeMath`, `sk_ieee_double_divide`,
`sk_double_saturate2int`, `SkTFitsIn`, `SkHalfToFloat`/`SkFloatToHalf`, `SkInvert{2x2,3x3,4x4}Matrix`,
`SkChecksum`), and §2.2 moves those down. The compiler never draws: it emits RP stages into a
sink (§6.3).

SkSL becomes a required dependency of core. PLAN §3.2's opt-in `sksl` feature is withdrawn,
because Skia's CPU build always contains SkSL and the default features (image filters, luma, blurs
in f16) need it. Only `skia-rust-sksl/wgsl` is optional.

### 2.2 `skia-rust-base`

S1 moves `math`, `floating_point`, `scalar`, `t_fits_in`, `to`, `safe_math`, `matrix_invert`,
`checksum` and `half` out of `skia-rust-core` (they form a closed set; `half` uses `simd::vx`) into
`crates/skia-rust-base`. In core they become `pub use skia_rust_base::<module>;`, which keeps
every path, test and `// Port of:` link unchanged. This mirrors Skia's own `src/base` bottom layer.
(The alternative, putting them in `skia-rust-simd`, avoids a crate but mixes general math into the
crate whose job is to fence `unsafe`; see Q1.)

## 3. Built-in modules

Skia keeps nine modules: `sksl_shared` (root intrinsics), `sksl_public` and `sksl_rt_shader`
(runtime effects), `sksl_gpu`, `sksl_frag`, `sksl_vert` and `sksl_compute` (GPU program kinds), and
the two Graphite modules. Each one inherits from its parent's symbol table
(`SkSLModuleLoader.cpp#L231-L330`). Each is compiled from text by `Compiler::compileModule` and
inlined after loading (`optimizeModuleAfterLoading`, `SkSLCompiler.cpp#L180-L211`, `#L315-L339`).
`ModuleLoader::Get()` is a process-wide, mutex-guarded, lazily filled cache.

**Decision: port the loader and compile the embedded texts at run time.**

- `crates/skia-rust-sksl/src/modules/original/sksl_*.sksl` holds the nine source files verbatim.
  `…/modules/minified/sksl_*.sksl` holds the nine minified texts, with their C++ string-literal
  wrapping undone. They are loaded with `include_str!("modules/…")` from `src/`: no `..` paths.
  Graphite's modules are compiled in but loaded only on request (`SkSLGraphiteModules`).
- `ModuleSource::{Minified, Original}` is part of the compiler's `Flavor` (decision 6).
  `ModuleLoader::get(source)` returns `&'static ModuleCache`, one `OnceLock<Arc<Module>>` per
  module and source. This memoizes a pure function of constant input, the same sanctioned exception
  as `StrikeCache::global()` (`docs/design/text.md` §5.2), and is recorded in `API_MAPPING.md`.
  Tests that build their own loader (as `sksl-minify` does with `ModuleLoader()`) get a fresh
  `ModuleCache` value.
- **Verification.** (a) `cargo xtask sksl sync-modules` copies and decodes both sets from
  `third_party/skia`. A test in the `inventory` CI job (the job that has the checkout) fails if they
  differ, so a pin bump cannot leave stale module text. (b) Every module compiles without errors
  in both variants, for every program kind (S11). (c) S24's `sksl-minify` port regenerates each
  `minified` text from its `original` byte for byte, which is the same check Skia's
  `minify_sksl` build step performs.

Why not run the compiler at build time and embed an IR dump: Skia does not do that at m156, so
nothing in Skia defines such a format. A `build.rs` that runs the compiler would also make every
compiler bug a build failure, and would double compile times. Compiling the modules once per process
costs milliseconds; Skia pays the same cost.

## 4. IR representation

### 4.1 Pools and ids

```rust
pub struct ExprId(u32);  pub struct StmtId(u32);  pub struct ElemId(u32);
pub struct SymId(u32);   pub struct TypeId(u32);  pub struct VarId(u32);  pub struct FnId(u32);
pub struct SymTabId(u32);

/// One module's or one program's IR. `parent` is the frozen pool it extends; ids below
/// `base` resolve in the parent chain.
pub struct IrPool {
    parent: Option<Arc<IrPool>>,
    base: Bases,                         // per-kind offsets = parent's lengths when frozen
    exprs: Vec<Expr>, stmts: Vec<Stmt>, elems: Vec<ProgramElement>,
    symbols: Vec<Symbol>,                // Variable, FunctionDeclaration, Type, Field, SymbolAlias, …
    symtabs: Vec<SymbolTable>,
}
```

- **Every node kind of Skia's IR exists from day one**, even where Phase 3 never emits it: compute,
  interface blocks, `inout`, mesh varyings and `layout` flags. The parser and the error goldens
  need all of them anyway, and the WGSL generator needs them later. Enums mirror Skia's `Kind`s:
  `ExprKind::{Binary, ChildCall, ConstructorArray, …, Swizzle, Ternary, VariableReference}`,
  `StmtKind::{Block, Break, Continue, Discard, Do, Expression, For, If, Nop, Return, Switch,
  SwitchCase, VarDeclaration}`, `ProgramElement::{Extension, Function, FunctionPrototype,
  GlobalVar, InterfaceBlock, ModifiersDeclaration, StructDefinition}`.
- `Expr { pos: Position, ty: TypeId, kind: ExprKind }`. Child edges are ids. Skia's `unique_ptr`
  child replacement is `pool.exprs[id] = new_node` at the same id. The inliner's
  `std::unique_ptr<Statement>*` candidates (`SkSLInliner.cpp`) become `StmtId`/`ExprId`, which stay
  valid across the analysis and rewrite phases exactly as Skia's slot pointers do. Replaced
  subtrees are left unreachable in the pool, as Skia's `Pool` leaves freed nodes. Programs are
  short-lived, and `RuntimeEffect` keeps only its base program.
- **Layering.** The root pool (built-in types from `SkSLBuiltinTypes.cpp`) is the bottom of every
  chain and is created in a fixed order, so `TypeId::FLOAT`, `TypeId::HALF4`, … are `const`s
  (`BuiltinTypes`). A module pool is frozen into an `Arc` after loading. A program pool has its
  module as parent and allocates new symbols locally, including array types created by
  `SymbolTable::addArrayDimension`. Lookups walk at most 5 parents. Frozen pools are immutable, so
  `Module` and `Program` are `Send + Sync` (`SkRuntimeEffectThreaded` needs that).
- **Equality is id equality** wherever Skia compares pointers (`Type` identity, `Variable*` keys in
  `ProgramUsage`, the RP generator's `fSlotMap`).
- Names are `Box<str>` owned by the symbol. Source text is `Arc<str>` in the `Program`, because
  `Position` holds byte offsets into it.

### 4.2 Context, errors, visitors

- Skia threads `const Context&` everywhere and mutates through it (errors, symbol table, pool).
  Here the conversion functions take `&mut Context`. `Context` holds the current `IrPool`, the
  `ErrorReporter`, the `ProgramConfig`, the current `SymTabId` and the caps. `Expression::Convert`
  / `Make` keep Skia's split: `convert` validates and reports, `make` asserts and folds.
- `ErrorReporter` is a trait (Skia's abstract class). `Compiler` implements it with `handleError`'s
  exact formatting. `NoOpErrorReporter` exists for analysis passes. Error messages are copied
  character for character from the C++.
- `ProgramVisitor` (read) and `ProgramWriter` (rewrite) become traits over `(&IrPool, id)` and
  `(&mut IrPool, id)`, with Skia's default recursion order. `clone()` deep-copies by id.
  `description()` is one printer per node with Skia's `OperatorPrecedence` handling. It is load
  bearing: error messages, `SkSLTest`'s `_Clone` check and `sksl-minify` all compare its text.
- Not ported: `SkSL::Pool`/`MemoryPool` (`fUseMemoryPool` is accepted and ignored),
  `SkSLCheckSymbolTableCorrectness` beyond `debug_assert!`s, and `SK_ENABLE_OPTIMIZE_SIZE`
  branches, because the oracle did not define it.

### 4.3 As implemented in S5

S5 landed the IR core in `crates/skia-rust-sksl`. This section is the contract for S6–S13:
where each Skia class lives, how to allocate and rewrite nodes, and the rules that keep the
layered pools sound. Where it differs from §4.1/§4.2, this section wins.

#### File map

| Skia | Rust | S5 has | Later tasks add |
|---|---|---|---|
| `ir/SkSLIRNode.h`, `SkSLExpression.*`, `SkSLStatement.h`, `SkSLProgramElement.h` | `ir/expression.rs`, `ir/statement.rs`, `ir/program_element.rs` | node structs, `*Kind` enums, `description`, `isIncomplete`, `isEmpty`, `asAnyConstructor` | `compareConstant`/`getConstantValue`/`supportsConstantValues` as `Expression` methods dispatching on the kind (S7a, S8) |
| `ir/SkSLBinaryExpression` | `ir/binary_expression.rs` | data, `description` | `convert`, `make`, `CheckRef`, `isAssignmentIntoVariable` (S7b) |
| `ir/SkSLConstructor*` (10 files) | `ir/constructor.rs` (the 9 payload structs + `any_constructor_description`) | data, `description` | `Constructor::Convert` as `constructor::convert`, each class's `convert`/`make` in a new file per Skia class (`ir/constructor_compound.rs`, …) with `impl ConstructorCompound` (S7a) |
| `SkSLEmptyExpression.h`, `SkSLFunctionReference.h`, `SkSLMethodReference.h`, `SkSLPoison.h`, `SkSLTypeReference.*` | `ir/simple_expressions.rs` | data, `description` | `make`/`convert`, `TypeReference::VerifyType` (S7b) |
| `SkSLPrefixExpression.*`, `SkSLPostfixExpression.*` | `ir/prefix_postfix.rs` | data, `description` | `convert`/`make` in `ir/prefix_expression.rs`, `ir/postfix_expression.rs` (S7b) |
| `SkSLFieldAccess`, `SkSLIndexExpression`, `SkSLSwizzle`, `SkSLTernaryExpression`, `SkSLVariableReference`, `SkSLSetting`, `SkSLLiteral`, `SkSLChildCall`, `SkSLFunctionCall` | `ir/<snake_name>.rs` | data, `description` (+ `Swizzle::mask_string`/`is_identity`, `CapsFlag` names, `Literal` accessors) | `convert`/`make` in the same file (S7a: literal; S7b: the others; S7c: calls) |
| `SkSLBlock` | `ir/block.rs` | data, `isEmpty`, `description` | `make`, `make_block`, `make_compound_statement` (S7d) |
| `SkSLBreakStatement.h`, `SkSLContinueStatement.h`, `SkSLDiscardStatement.*`, `SkSLNop.h`, `SkSLReturnStatement.h` | `ir/simple_statements.rs` | data, `description` | `DiscardStatement::convert` (S7d) |
| `SkSLDoStatement`, `SkSLExpressionStatement`, `SkSLForStatement`, `SkSLIfStatement`, `SkSLSwitchStatement`, `SkSLSwitchCase` | `ir/control_statements.rs` | data, `description`, `LoopUnrollInfo`, `SwitchStatement::cases` | `convert`/`make` in a new file per class (`ir/for_statement.rs`, …) (S7d) |
| `SkSLVarDeclarations` | `ir/var_declarations.rs` | `VarDeclaration`, `GlobalVarDeclaration`, `description` | `ErrorCheck`, `convert`, `make` (S7d) |
| `SkSLExtension.h`, `SkSLFunctionDefinition.h`, `SkSLFunctionPrototype.h`, `SkSLInterfaceBlock.*`, `SkSLModifiersDeclaration.h`, `SkSLStructDefinition.*` | `ir/program_element.rs` | data, `description` | `convert`/`make` in a new file per class (S7c: function definition; S7d: the rest) |
| `SkSLType` | `ir/types.rs` | `Type` (all subclasses as `TypeClass`), `TypeRef` with every virtual accessor (`componentType`, `columns`, `slotType`, `isAllowedInES2`, …), `Field`, `StructType::new`, `CoercionCost`, `getArrayName` | `coercionCost`, `toCompound`, `applyQualifiers`, `coerceExpression`, `checkForOutOfRangeLiteral`, `checkIfUsableInArray`, `convertArraySize`, `isAllowedInES2(context)`, `Type::clone`, the checked `MakeArrayType`/`MakeStructType` (S6) |
| `SkSLBuiltinTypes` | `builtin_types.rs` | the table and the `TypeId` constants | — |
| `SkSLSymbol`, `SkSLSymbolTable` | `ir/symbol.rs`, `ir/symbol_table.rs` | `SymbolKind`; name/position/type/description by `SymbolId`; table storage, `find` (`IrPool::find_symbol`), `findBuiltinSymbol`, `isType`, `isBuiltinType`, `injectWithoutOwnership` (`inject_symbol`) | `add`/`addWithoutOwnership` (with errors), `renameSymbol`, `removeSymbol`, `moveSymbolTo`, `insertNewParent`, `addArrayDimension`, `instantiateSymbolRef`, `wouldShadowSymbolsFrom` (S6); `Symbol::instantiate` (S7b) |
| `SkSLVariable`, `SkSLFieldSymbol.h`, `SkSLFunctionDeclaration` | `ir/variable.rs`, `ir/field_symbol.rs`, `ir/function_declaration.rs` | data, declaration links, `description`; `FunctionDeclaration` has no constructor yet (tests use the struct literal) | `Variable::convert`/`make`/`MakeScratchVariable` (S6); the `FunctionDeclaration` constructor (main-parameter rules), `convert`, `mangledName`, `matches`, `determineFinalTypes` (S7c) |
| `SkSLLayout`, `SkSLModifierFlags`, `SkSLModifiers.h` | `ir/layout.rs`, `ir/modifier_flags.rs` | `LayoutFlags`/`ModifierFlags` (bitflags), `Layout`, `Modifiers`, both `description`s | `checkPermittedLayout`, `checkPermittedFlags` (S6) |
| `SkSLProgram`, `SkSLModule.h` | `ir/program.rs`, `modules.rs` (`Module`; `ModuleType` gained `Program` and `Unknown`) | `Program` (`elements`, `getFunction`, `description`), `ProgramInterface`, `UniformInfo` | `usage` (S9a), construction (S11) |
| `SkSLContext`, `SkSLErrorReporter`, `SkSLCompiler.cpp#L441-L537` | `context.rs`, `error_reporter.rs`, `compiler.rs` | all of it | `Compiler`'s module loading, `convert_program`, `optimize`, `finalize` (S11, S13) |
| `SkSLMangler`, `SkSLProgramSettings.h`, `SkSLProgramKind.h`, `SkSLDefines.h`, `SkSLIntrinsicList` | `mangler.rs`, `program_settings.rs`, `defines.rs`, `intrinsic_list.rs` | all of it | — |
| `analysis/SkSLProgramVisitor.h`, `transform/SkSLProgramWriter.h` | `analysis/program_visitor.rs`, `transform/program_writer.rs` | the traits and Skia's default recursion | the analyses (S9a/b) and transforms (S11, S13) next to them |

#### Pools, ids and layering

- Ids: `ExprId`, `StmtId`, `ElemId`, `TypeId`, `VarId`, `FnId`, `FieldId` (anonymous interface
  block fields), `SymTabId`, all `Copy`. `SymbolId` is the enum Skia's `Symbol*` becomes:
  `Type(TypeId) | Variable(VarId) | FunctionDeclaration(FnId) | Field(FieldId)`. Skia's
  `SymbolKind::kExternal` has no node at m156 and is not represented.
- Symbols are not one arena: each kind has its own (`pool.variable(id)`, `pool.function(id)`,
  `pool.type_node(id)`, `pool.field_symbol(id)`), so typed ids need no runtime kind checks.
- Every arena has the same API: `pool.expression(id)` (resolves through the parent chain),
  `pool.expression_mut(id)` (local nodes only; **panics on a frozen parent's id**),
  `pool.add_expression(node) -> ExprId`, `pool.next_expression_id()`,
  `pool.is_local_expression(id)`. Likewise `statement`, `element`, `variable`, `function`,
  `field_symbol`, `symbol_table`, and `type_mut`/`add_type`.
- **Built-in types** are a `static` table below every pool; their ids are the constants in
  `builtin_types.rs`, in `BuiltinTypes`' constructor order. Naming rule: Skia's field minus the
  `f`, in SCREAMING_SNAKE (`fFloat2x2` → `TypeId::FLOAT2X2`, `fUInt` → `UINT`, `fIVec2` →
  `IVEC2`, `fGenHType` → `GEN_HTYPE`, `fSkCaps` → `SK_CAPS`, `fTexture2D_sample` →
  `TEXTURE2D_SAMPLE`); the one clash is `fAtomic_uint` → `ATOMIC_UINT_ALIAS`. Every constant
  has `#[doc(alias = "fName")]`.
- **Layering.** `IrPool::new()` is a root pool (used for `sksl_shared` and in tests).
  `IrPool::extend(parent: Arc<IrPool>)` layers a new pool on a frozen one; `pool.freeze()`
  gives the `Arc`. S11: compile a module into `IrPool::extend(parent_module.pool.clone())`
  (root for the first), freeze it into `Module { parent, pool, symbols, elements, module_type }`,
  and compile programs into `IrPool::extend(module.pool.clone())`. A program's symbol table
  is a local `SymbolTable` whose `parent` is the module's table id and which has
  `at_module_boundary` set.
- **Frozen means immutable.** Shared IR never changes after its module loads. Every Skia
  mutation we checked touches only the node being created (`setNextOverload` on the new
  declaration, `setDefinition` on program functions, `setVarDeclaration` on the new variable).
  If a port hits the "frozen parent pool" panic, the C++ is mutating shared state: stop and
  look, do not work around it.
- **One owner per id.** A node id appears in exactly one parent slot. Skia's `std::move` of a
  `unique_ptr` into a new parent is "copy the id into the new parent and stop using the old
  parent". Never put the same `ExprId` into two parents; clone instead. Replaced and abandoned
  nodes stay in the pool, unreachable (as Skia's `Pool` keeps freed nodes until the program
  dies).
- **Rewriting a slot.** Skia's `std::unique_ptr<Expression>& slot` is the id. `slot = newNode`
  is `pool.replace_expression(slot, node)`; `slot = std::move(child)` (child of the node in
  `slot`) is `pool.move_expression_into(slot, child)`; wrapping a slot's node in a new parent
  (the inliner's `block->children().push_back(std::move(*stmt)); *stmt = std::move(block)`) is
  `let moved = pool.relocate_statement(slot); /* build block with children [.., moved] */
  pool.replace_statement(slot, block)`. Ids held across a rewrite keep naming the same slot,
  which is what the inliner's `std::unique_ptr<Statement>*` candidates rely on.
- **Nullable slots** are `Option<…>` (`ForStatement::{initializer, test, next}`,
  `IfStatement::if_false`, `ReturnStatement::expression`, `VarDeclaration::value`). Arrays
  (`ExpressionArray`, `StatementArray`) are `Vec<ExprId>`/`Vec<StmtId>` with no null entries:
  Skia only nulls entries of arrays it is about to discard.
- `FunctionCall::stable_pointer` is the id the call was first allocated at: take
  `pool.next_expression_id()` right before `add_expression`. `clone` copies it, as Skia does.
- Skia's back-pointers become ids: `Variable::declaring_element`
  (`DeclaringElement::VarDeclaration(StmtId) | GlobalVarDeclaration(ElemId)`),
  `Variable::interface_block`, `FunctionDeclaration::{definition, next_overload}`. Skia's
  `detachDead*` destructor hooks have no equivalent (nothing is destroyed).

#### Nodes and how to write `Convert`/`Make`

- Node = `Expression { position, ty, kind }`, `Statement { position, kind }`,
  `ProgramElement { position, kind }`. `kind` is `ExpressionKind::Binary(BinaryExpression)`,
  `StatementKind::For(ForStatement)`, `ProgramElementKind::GlobalVar(GlobalVarDeclaration)`, …
  in Skia's `Kind` order; leaf classes are unit structs (`Poison`, `Nop`, `BreakStatement`, …).
  Payload fields are `pub` and named after Skia's accessors in snake case (`if_true`,
  `field_index`, `stable_pointer`; `type()` is `ty`, `MethodReference::self()` is `self_`).
- A Skia factory is an associated function of the payload struct:
  `impl BinaryExpression { pub fn convert(ctx: &mut Context, pos: Position, left: ExprId, op:
  Operator, right: ExprId) -> Option<ExprId>; pub fn make(ctx: &mut Context, …) -> ExprId }`.
  Map the C++ types as: `std::unique_ptr<Expression>` (owned) → `ExprId`; `nullptr` result →
  `None`; `ExpressionArray` → `Vec<ExprId>`; `const Type&`/`const Type*` → `TypeId`;
  `const Variable*` → `VarId`; `const FunctionDeclaration*` → `FnId`; `SymbolTable*` →
  `SymTabId`; `const Context&` → `&mut Context` (errors and allocation go through it). To
  build a node: `ctx.pool.add_expression(Expression::new(pos, ty, ExpressionKind::…))`.
- **Borrowing.** A `&Expression` borrowed from `ctx.pool` cannot live across a call that takes
  `&mut Context`. Copy what you need first (ids are `Copy`; clone a payload if needed), or
  borrow fields separately: `ctx.pool` and `ctx.errors` are disjoint, so
  `let e = ctx.pool.expression(id); ctx.errors.error(e.position, "…")` compiles. Functions that
  need the context therefore take ids, not node references (`Expression::is_incomplete(ctx,
  id)` is the model).
- **Types.** `ctx.pool.ty(id)` returns a `TypeRef`, which derefs to `Type` (`name`,
  `abbreviated_name`, `type_kind`, `class`) and answers every virtual query, following aliases
  exactly as Skia's overrides do. Pointer comparisons (`&type == fContext.fTypes.fFloat.get()`)
  become `ty == TypeId::FLOAT`; `type.matches(other)` becomes `pool.ty(a).matches(b)`. Keep
  Skia's quirks: `numberKind()` of a vector or matrix is `Nonnumeric` (so
  `pool.ty(TypeId::FLOAT4).is_float()` is false; use `component_type()`). Array and struct
  types are built with `Type::new_array_type` / `Type::new_struct_type(StructType::new(…))`
  and allocated with `ctx.pool.add_type`; S6's `MakeArrayType`/`MakeStructType`/
  `addArrayDimension` wrap them with Skia's checks.
- **Errors.** `ctx.errors.error(pos, &msg)`, with the message copied character for character.
  Messages that contain `<POISON>` are dropped by the reporter, as in Skia.

#### `description()` and `clone()`

- `pool.expression_description(id)`, `pool.expression_description_with(id, precedence)`,
  `pool.statement_description(id)`, `pool.element_description(id)`,
  `pool.symbol_description(symbol)`, `pool.ty(id).description()`, `Program::description()`.
  Every node's printer is ported from its Skia override, quirks included (`StructDefinition`
  prints `struct S {  float x; };`, an `InterfaceBlock` glues its layout to its modifiers).
- Literal floats print through `skstd::to_string_f32` because `SKSL_FLOAT` is `float` at m156
  (`SkSLDefines.h#L21`; §5 said `double` and is corrected). Constant folding follows each
  fold's own C++ types.
- Verified by `ir/tests.rs`: hand-built IR reproduces `folding/ArraySizeFolding`,
  `folding/TernaryFolding` and `runtime/ChildEffectSimple` `.minified.sksl` exactly (after the
  minifier's whitespace stripping), the `for` loop of `runtime/ArrayIndexing`, and the function
  descriptions quoted in `errors/Ossfuzz38140` and the intrinsic-redefinition errors.
  `compiler.rs` reproduces the error text of `errors/Ossfuzz38140`, `ForLoopOverflow`,
  `IllegalRecursionSimple` and `Ossfuzz44561` byte for byte.
- `pool.clone_expression(id)` / `clone_expression_at(id, pos)` / `clone_expression_array`:
  deep copy into the calling pool (also of nodes that live in a parent, which is how the inliner
  copies module code into a program). Skia has no `Statement::clone`; the inliner (S12) builds
  its statement copies itself with `add_statement`, reading module nodes freely.

#### Context, errors and compiler

- `Context { config: Option<ProgramConfig>, errors: ErrorReporter, module: Option<Arc<Module>>,
  symbol_table: Option<SymTabId>, pool: IrPool }`. `ctx.config()` unwraps the config.
  `Compiler` owns the context. A finished `Program` owns its pool; passes that run on a program
  use `ctx.with_program(&mut program, |ctx| …)`, which lends the program's pool, config and
  symbol table to the context and takes them back.
- `ErrorReporter` is one struct with an `ErrorSink` enum instead of Skia's subclasses:
  `Compiler { error_text }` (formats with `compiler::handle_error`), `Forwarding { errors }` (the
  parser checkpoint's reporter), `NoOp`, `TestingOnlyAbort`. The parser's
  `context.setErrorReporter(&fErrorReporter)` is `let old =
  ctx.set_error_reporter(ErrorReporter::forwarding())`, and restoring is
  `let forwarding = ctx.set_error_reporter(old)` followed by forwarding its errors (S10).
  The compiler's source text is `ctx.errors.set_source(Arc<str>)`.
- `Compiler::error_text(show_count)` (and `error_text_bytes` for exact bytes),
  `write_error_count`, `reset_errors`, `error_count`, `Compiler::POISON_TAG`.

#### Visitors and writers

- `ProgramVisitor` methods take `(&mut self, pool: &IrPool, id)`. Override a method and call
  `walk_expression`/`walk_statement`/`walk_program_element` for Skia's `INHERITED::visitX`.
  `visit(&program)` visits shared elements first, then owned ones.
- `ProgramWriter` methods take `(&mut self, ctx: &mut Context, id)`; the walkers copy the child
  ids out before recursing, so overrides may rewrite through `ctx.pool`. Override
  `visit_expression_ptr`/`visit_statement_ptr` to replace a child (Skia's `unique_ptr&`
  overloads) and rewrite with `replace_expression`/`move_expression_into`. Writers only run on
  a program's owned elements; shared elements are frozen.

#### Notes for the next tasks

- **S6** ports into `ir/types.rs` (as `impl TypeRef` or `impl Type` methods taking ids and
  `&mut Context`), `ir/symbol_table.rs` (as `IrPool`/`Context` methods taking a `SymTabId`:
  `add` needs the context for its duplicate-symbol error, so it is
  `fn add_symbol(ctx: &mut Context, table: SymTabId, symbol: SymbolId)`), `ir/layout.rs`,
  `ir/modifier_flags.rs` and `ir/variable.rs`, and creates `util.rs` (`ShaderCaps`,
  `ShaderCapsFactory`) and `memory_layout.rs`. `toCompound(context, columns, rows)` returns a
  built-in `TypeId` picked by `match` on the component's id, mirroring Skia's switch.
  `SymbolTable`'s `std::collections::HashMap` is fine: Skia only iterates it in the unported
  `CheckSymbolTableCorrectness`.
- **S7a–d** follow the file map and the `convert`/`make` conventions above. A `Make` that
  returns one of its arguments unchanged returns that `ExprId` (after setting its position
  with `ctx.pool.expression_mut(id).position = pos` where Skia does `expr->fPosition = pos`).
  `Symbol::instantiate` becomes `fn instantiate(ctx: &mut Context, symbol: SymbolId, pos:
  Position) -> Option<ExprId>` in `ir/symbol.rs`.
- **S9a** keys `ProgramUsage` by `VarId`/`FnId`. Use `std` maps only where Skia never iterates
  in an order-visible way; otherwise `thash` (§5, R2).
- **S11** builds the module chain as described under "Layering", sets
  `ctx.errors.set_source`, and moves the finished pool into `Program`.
- **S12** keeps its candidates as `StmtId`/`ExprId` slots and uses the relocate/replace idioms;
  `Mangler::unique_name(base, &ctx.pool, symbol_table)` matches Skia's counter and truncation.

### 4.4 As implemented in S6

S6 landed the type, symbol, declaration, layout and memory rules in `crates/skia-rust-sksl`. The
conventions of §4.3 hold, plus these:

- **Methods that report errors take `&mut Context`** and sit on `TypeId`: `apply_qualifiers`,
  `clone_in`, `check_if_usable_in_array`, `check_for_out_of_range_literal_value`,
  `convert_array_size_value`. Pure queries sit on `TypeRef`: `coercion_cost`, `can_coerce_to`,
  `to_compound`, `is_allowed_in_es2_for`. The checked factories are `Type::make_array_type` and
  `Type::make_struct_type`.
- **Symbol-table operations are free functions** in `ir/symbol_table.rs`, re-exported from `ir`:
  `add_symbol(ctx, table, symbol)` (reports duplicates, returns nothing), `rename_symbol`,
  `remove_symbol`, `move_symbol_to`, `insert_new_parent`, `add_array_dimension` (returns the
  `TypeId`), `would_shadow_symbols_from`. Ownership is the pool's, so `add` and
  `addWithoutOwnership` are one function, and `removeSymbol` returns nothing.
- `Variable::convert(ctx, …) -> VarId` validates and allocates; `Variable::make(pool, …) -> VarId`
  allocates. `Layout::check_permitted_layout`, `ModifierFlags::check_permitted_flags`,
  `Operator::determine_binary_type(ctx, left, right) -> Option<BinaryTypes>` and
  `Operator::is_matrix_multiply(pool, …)` are in place.
- `util::ShaderCaps` (its GLSL-only fields are not ported), `ShaderCapsFactory::{standalone,
  default_caps}` (cached in `OnceLock`s), and `memory_layout::MemoryLayout` with
  `Standard::{Std140, Std430, Metal, WgslUniformBase, …}`.
- `Compiler::FRAGCOLOR_NAME` joins `POISON_TAG` in `compiler.rs`.

**Deferred, as of S6.** These needed the S7a constructors, the S7b operators or the S8 constant
folder. The first two are done (see below). The other two are still open:

- `Variable::MakeScratchVariable`: needs `VarDeclaration::Make` (S7d).
- `type_to_sksltype`: belongs with the runtime-effect uniform types (S18).

**File ownership for the parallel tasks.** Each file has one owner, except the shared hot spots,
which are listed separately.

| Task | Owns |
|---|---|
| S7a | `ir/constructor*.rs`, `ir/literal.rs`, and a `coerce_expression` addition to `ir/types.rs` |
| S7b | `ir/binary_expression.rs`, `ir/prefix_postfix.rs`, `ir/ternary_expression.rs`, `ir/index_expression.rs`, `ir/swizzle.rs`, `ir/field_access.rs`, `ir/variable_reference.rs`, `ir/setting.rs`, `ir/simple_expressions.rs`, `ir/child_call.rs`, `Symbol::instantiate` in `ir/symbol.rs`, `instantiate_symbol_ref` in `ir/symbol_table.rs` |
| S7c | `ir/function_call.rs`, `ir/function_declaration.rs`, `intrinsic_list.rs`, the function-definition part of `ir/program_element.rs` |
| S7d | `ir/block.rs`, `ir/control_statements.rs`, `ir/simple_statements.rs`, `ir/var_declarations.rs`, the other element kinds of `ir/program_element.rs`, and new per-class files |
| S8 | `constant_folder.rs` (new) |

Shared hot spots: `ir/mod.rs` (module lines and re-exports; S7a–d all add some), and
`ir/program_element.rs` (S7c and S7d both edit it). Agree on one edit order, or rebase the second
PR onto the first. `compiler.rs` belongs to S11.

### 4.5 As integrated in S5–S9 (`port/sksl-5`)

The S7a, S7b, S7c, S8, S9a and S9b branches are merged into one tree, and the stand-ins those
tasks left for each other are gone. Every call goes to the function that owns it:

- **Deferred from S6, now done.** `Type::coerce_expression` (`types.rs`),
  `check_for_out_of_range_literal`, `convert_array_size` (`types.rs`, with
  `constant_folder::get_constant_int`), and `SymbolTable::instantiate_symbol_ref` (`Symbol::instantiate`).
- **S7b.** `ir/constant_folder_stub.rs` and `ir/s7b_shims.rs` are deleted. The operators call the
  S7a `make`s, `Type::coerce_expression`, `constant_folder::{simplify, get_constant_value_for_variable}`
  and `analysis::{is_assignable, update_variable_ref_kind}`. The folding of `-x`, `!b` and `~x`
  (`SkSLPrefixExpression.cpp`) is complete in `ir/prefix_postfix.rs`.
- **S8.** The private node builders and constant-value helpers are gone. The folder uses
  `Literal::make*`, `ConstructorSplat/DiagonalMatrix/Array/Compound::make` (and
  `make_from_constants`), `BinaryExpression::make*`, `PrefixExpression::make`,
  `Expression::{get_constant_value, compare_constant, supports_constant_values}` and the S9a
  analyses.
- **S7a.** `ir/constructor.rs` has no stand-ins. Its constant-value methods are the ones S8 uses.
  Skia m156 overrides `getConstantValue`/`supportsConstantValues` only in `Literal` and the
  constructors (`SkSLLiteral.h`, `SkSLConstructor*.h`); `IndexExpression`, `PrefixExpression` and
  `FunctionCall` have no override, so they stay on the default (no constant value).
- **S9b.** `analysis/s9b_shims.rs` is deleted. `WriteCounts` is `ProgramUsage` (`get_variable(v).write`),
  the constant-expression visitor with loop indices is `expression_queries::is_constant_expression_with_loop_indices`,
  and `is_same_expression_tree`, `statement_writes_to_variable` and `BinaryExpression::make` are the
  S9a and S7b functions. `SymbolTableStackBuilder` has one definition (S9a); its method is `finish`.
  `SafeMath`, `saturating_add_size` and `double_saturate2int` moved to `base_helpers.rs`, and
  `forward_errors` to `error_reporter.rs`.
- **S7c.** No `pending()` path is left. `Constructor::Convert` is `constructor::convert`, argument
  coercion and out-parameter reference kinds are real, `eval()` becomes `ChildCall::make`, and
  the sk_Position fixup is built with the `IRHelpers` members it uses (local to
  `ir/function_definition.rs`). `ExpressionStatement::make` is ported in `ir/control_statements.rs`
  for the fixup. Intrinsic folding (`SkSLFunctionCall.cpp`, the `Intrinsics` namespace and
  `optimize_intrinsic_call`) is `ir/function_call_intrinsics.rs`.
- **S9a.** `analysis/returns_opaque_color.rs` ports `ReturnsNonOpaqueColorVisitor`.
- **Tree-shape optimizations** (S7b's deferred list): the ternary `fOptimize` rewrites
  (`ir/ternary_expression.rs`), constant array and matrix index extraction in
  `IndexExpression::Make`, `optimize_constructor_swizzle` in `Swizzle::Make`, struct-constructor
  field extraction in `FieldAccess::Make`, and constructor-cast folding (S7a's
  `ConstructorScalarCast::make`).
- **Copies of skia-rust-base helpers** that the sksl crate needs (it does not depend on
  skia-rust-core): `base_helpers.rs` (`SafeMath`, `ieee_double_divide`, half floats, `is_finite_array`)
  and `matrix_invert.rs` (the 2x2, 3x3 and 4x4 inverses). Both go away when skia-rust-base lands.
- `intrinsic_list.rs` gained `not`, which Skia's `SKSL_INTRINSIC_LIST` has (106 entries).

### 4.6 As implemented in S10

`crates/skia-rust-sksl/src/parser.rs` ports `SkSLParser.{h,cpp}` (m156) function by function.

- **API.** `Parser::new(&mut Context, ProgramSettings, ProgramKind, &[u8])` borrows the context
  (Skia's `Compiler&`) and the source bytes. The context needs `config`, a current `symbol_table`
  (the module's or program's global table, created with `mark_module_boundary`), and the error
  reporter with `set_source`/`set_source_bytes`. The source is bytes because Skia's strings are
  (fuzzer inputs need not be UTF-8); `ErrorReporter::source` is now `Arc<[u8]>`.
- **Driver hooks for S11.** Skia's two entry points end in compiler code, so they stop at the parse:
  `Parser::program_inheriting_from(self) -> Option<Vec<ElemId>>` (`None` when errors were
  reported; S11 does `Compiler::releaseProgram`: build the `Program`, then `finalize` and
  `optimize`) and `Parser::module_inheriting_from(self) -> Vec<ElemId>` (S11 wraps the elements,
  the global symbol table and the parent in a `Module`). `initializeContext`/`cleanupContext` and
  `FinalizeSettings` are S11's: `tests/parser_errors.rs` has a small version of each (root symbol
  table, the module chain compiled with this parser from the original module texts,
  `allow_narrowing_conversions` for runtime effects) that S11 can lift.
- **Mechanics.** `AutoDepth` is `auto_depth(|this| ..)` plus `increase_depth()` (the depth is
  restored when the closure returns, as the destructor does); `AutoSymbolTable` is
  `with_symbol_table(enable, |this, table| ..)`; `Checkpoint` swaps in `ErrorReporter::forwarding()`
  and `accept`/`rewind` restore the old reporter (forwarding the collected errors on accept). The
  function body block adopts the function's table (`block(false, Some(table))`). The binary
  precedence levels share `binary_level`/`binary_level_ops`, which are Skia's loops.
- **Tests.** `tests/parser_errors.rs` runs the 333 `resources/sksl/errors` inputs against their
  `tests/sksl/errors/*.glsl` goldens byte for byte, and the 38 `runtime_errors` inputs with
  `SkSLErrorTest`'s rule (every expected message appears, in order). Cases whose messages come
  from `finalize`, the inliner or `SkRuntimeEffect` validation are listed in
  `DEFERRED_TO_FINALIZATION`/`DEFERRED_RUNTIME_ERRORS`; the test checks that the parser accepted
  them, and S11 should delete those lists as it ports the phases. One golden quotes a non-UTF-8
  source byte (`Ossfuzz519154489`); its messages are `&str`, so that case compares after lossy
  conversion. The data is vendored under `tests/data` (`-text` in `.gitattributes`, since some
  inputs hold CR bytes).
- **Fixes in shared code that the parser exposed.** `string::stoi` returns an `SKSL_INT` (an
  `i64`: `4294967295` stays positive), `string::stod_float` parses straight to `f32`
  (`SKSL_FLOAT`), `Literal::make_int_literal` has no range assert (Skia's context overload
  has none), and `slotCount` arithmetic wraps like `size_t`. The `Finalizer` and
  `Type::checkForOutOfRangeLiteral` no longer ask an unsized array for its slot count, and the
  compound-constructor check no longer asks a non-vector argument for its columns.

### 4.7 As implemented in S11

The driver is in `compiler.rs` (`Compiler`, with a `Flavor` chosen at construction), and the
module chain is in `module_loader.rs`.

- **Driver.** `finalize_settings` (`FinalizeSettings`, without Skia's static overrides),
  `initialize_context`/`cleanup_context`, `module_for_program_kind`, `compile_module` (and
  `compile_module_parts`, which the loader uses so it can add the public aliases before the pool
  is frozen), `convert_program` (the parse, then `release_program`), `finalize` and `optimize`.
  `finalize` runs the three `FindAndDeclareBuiltin*` transforms (`transform/find_and_declare.rs`),
  then `do_finalization_checks`, the strict-ES2 indexing check and `check_program_structure`, in
  Skia's order, inside `Context::with_program`.
- **Modules.** `ModuleLoader::for_flavor` gives one loader per flavour. Each module (root, shared,
  gpu, frag, vert, compute, public, rt_shader) is a `OnceLock`, compiled from its flavour's text.
  `compile_and_shrink` drops the function prototypes and adds the public type aliases to
  `sksl_public` (`addPublicTypeAliases`). `Module` keeps its source text.
- **Bytes end to end.** `ErrorReporter::error_bytes`, `Parser::error_bytes` and `Program::source`
  are bytes, so `Ossfuzz519154489` (a message quoting byte `0xFF`) compares exactly.
  `ErrorReporter::error(&str)` stays for the messages that are text.
- **skslc.** `tests/src/tools/skslc.rs` compiles with `Flavor::Standalone`. A program that fails
  to compile gives `### Compilation failed:` and the error text, which is the golden. A program
  that compiles needs a code generator, so it returns `NotPorted`. The golden runner reads inputs
  as bytes.
- **Deferred.** `tests/parser_errors.rs` compiles every `errors/*.glsl` input and compares the
  bytes. Four goldens (`ArrayInlinedIndexOutOfRange`, `MatrixInlinedIndexOutOfRange`,
  `VectorInlinedIndexOutOfRange`, `OverflowInlinedLiteral`) come from the inliner (S12): the test
  requires that the front end accepts them. `SamplerExternalOES` comes from the GLSL generator
  (`ShaderCaps::fExternalTextureSupport`, R7): excluded in the manifest.
- **Hooks for S12 and S13.** `Compiler::optimize_module_after_loading` runs the module inliner
  after the module is parsed and outside its context window (Skia's `AutoProgramConfig` set-up
  belongs in S12). `Compiler::run_optimizer_passes` is the program optimizer: S12's inliner, then
  S13's `EliminateUnreachableCode` and the dead-function and dead-variable passes, in Skia's order.
  Both run only when `ProgramSettings::optimize` is set (`Compiler::optimize`). `Program` keeps no
  cached usage: `analysis::get_usage` computes it, and the transforms update a local copy.

## 5. Exactness requirements

| Area | Requirement | Where it shows |
|---|---|---|
| Error text | `handleError` formatting (`error: ` + line + `: ` + msg, echo with the 100-char window and `...`, carets over `[start, end)` clamped to the line, `Position` length capped at 255), `errorText()`/`writeErrorCount()` (`N error` / `N errors`), the order in which errors are reported (conversion order, then finalization). Positions are byte offsets; the line is the count of `\n` | `errors/*.glsl` (338), 106 + 7 + 7 failure goldens, `SkSLErrorTest`, `SkRuntimeEffectTest` (`errorText` substrings) |
| Literals | `skstd::to_string(float/double)` = iostream `%g` with precision 7, then 9/17 if the value does not round-trip, then append `.0` when there is no `.`/`e` (`SkSLString.cpp#L23-L56`). Port the `%g` algorithm: Rust has no `%g`, so build it from Rust's correctly rounded `{:.*e}`. `SkSL::stod` = `istream >> double` (correctly rounded; `str::parse::<f64>`), `stoi` = `strtoull(base 0)`, so `010` is octal, plus `u` suffix and `≤ 0xFFFFFFFF` | every generator's output, `.skrp` `0x3F800000 (1.0)` |
| Constant folding | Literals hold a `double`; `SKSL_FLOAT` (what `Literal::floatValue()` returns) is `float` (corrected in S5, §4.3). Each fold uses the C++ types of its own code, with Skia's range checks and casts. Intrinsic folds call `std::sin`/`pow`/… in double (`SkSLFunctionCall.cpp`): use the host `f64` functions with `// skia-rust: libm` (R4) | `.skrp`, `.wgsl`, `.minified.sksl` |
| Optimizer | Pass order and repeat loops exactly as `optimize`/`finalize`/`optimizeModuleAfterLoading`/`optimizeModuleBeforeMinifying` (`SkSLCompiler.cpp#L253-L440`), `FinalizeSettings` (`#L89-L125`), the `getRPProgram` re-compile with `kDefaultInlineThreshold` (`SkRuntimeEffect.cpp#L218-L284`) | all outputs |
| Inliner / names | Candidate discovery order, `Mangler` counters (`_0_x`, `_1_y`, per inliner), `fInlinedStatementCounter`, `RenamePrivateSymbols`' naming sequence | `.skrp` slot names, WGSL, minified |
| Hash iteration | `findPreexistingImmutableData` iterates `THashSet<Slot>` (`SkSLRasterPipelineCodeGenerator.cpp#L2735-L2780`), and the first match decides which immutable slots are reused. Port `SkTHashTable` (open addressing, `SkGoodHash` = `SkChecksum::Mix` for 4-byte keys, growth policy, slot-order iteration) as `skia_rust_sksl::thash`. Every other Skia hash map in the ported code is lookup-only or sorted before output (WGSL's field polyfills, `#L1470-L1484`); `std` maps are fine there, and each one that iterates needs a comment that says so | `.skrp` immutable ranges (`i3..4`) |
| RP builder | Every peephole in `Builder` (`simplifyImmediateUnmaskedOp`, `simplifyPopSlotsUnmasked`, `discard_stack` merges, immediate-op conversion, `push_duplicates`, `swizzle` shortcuts, …), label numbering, `finish()`'s stack-depth accounting, `makeStages`' op selection (`appendCopy*`, n-way vs immediate vs multi-slot) | `.skrp`, `RasterPipelineBuilderTest` |
| Dump | `Program::Dumper` text (op names padded to 30, `$n`/`vN`/`iN`/`uN` ranges, `imm()` formatting, branch `+N (label L at #K)`, the unique-name subscripts `₀₁…` in UTF-8), with or without the instruction count | `.skrp`, `RasterPipelineBuilderTest` |
| Flavor | Standalone: rewinds in `makeStages`, original modules. Library: minified modules, no rewinds | §1.3 |
| Lane count | `allocateSlotData`/`makeStages` use `N = selection().tier.highp_stride()` (Skia: `SkOpts::raster_pipeline_highp_stride`) | pixels on every tier |

## 6. The Raster Pipeline back end

### 6.1 `Builder` and `Program`

`RP::Builder` (`Instruction` list, `BuilderOp`, labels, temp stacks) is a value type with no
external dependency: S14 ports it 1:1. `Program` holds the instructions, slot counts and
`Arc<DebugTracePriv>` (slot names are needed for dumps even without tracing). `ProgramOp` and
`BuilderOp` extend `rp::Op`'s list with Skia's extended ops (`label`, `invoke_*`), reusing the
`rp_ops!` table so the discriminants match (`static_assert((int)ProgramOp::label ==
(int)BuilderOp::label)`).

### 6.2 `makeStages`: from instructions to `Stage<'a>`

`make_stages` returns `Vec<ProgramStage<'a>>` (`Stage<'a>` or one of `Label(i32)`,
`InvokeShader(i32)`, `InvokeColorFilter(i32)`, `InvokeBlender(i32)`, `ToLinearSrgb(MemPtr)`,
`FromLinearSrgb(MemPtr)`, `StackRewind`), exactly Skia's `TArray<Stage>`. Contexts are the typed
simd contexts. Offsets are bytes from the slab's `MemPtr`. Uniform data lives in the slab too (the
uniform block, §6.5), so the uniform contexts hold `MemPtr`s like the others. Pointer packing
(`SkRPCtxUtils::Pack`) is not observable with typed contexts and is not ported.

Slot memory: `allocateSlotData` (`SkSLRasterPipelineBuilder.cpp#L1675-L1695`) becomes
`alloc.alloc_scratch_init(bytes, init)`, a new `ArenaAlloc` method that reserves scratch bytes
together with their initial contents. The immutable slots are written when the stages are built
(`#L1871-L1876`), and the code that binds `SHADER_SCRATCH` (blitter, `ColorFilter::filter_color4f`,
image-filter backends) copies the recorded initial image into the buffer it zeroes today. Skia
allocates the slab once per `appendStages` and keeps it across `run`s of a compiled pipeline, and
the blitter's scratch buffer has the same lifetime, so stale values carry over between runs exactly
as in Skia.

### 6.3 `appendStages`

Skia's `Program::appendStages(SkRasterPipeline*, SkArenaAlloc*, Callbacks*, SkSpan<const float>)`
(`#L1697-L1819`) appends to core's pipeline and calls back into core for children. Core sits above
sksl, so sksl declares the two traits it needs, and core implements them:

```rust
pub trait StageSink<'a> {                     // implemented by core's RasterPipeline<'a>
    fn append(&mut self, stage: Stage<'a>);
    fn append_stack_rewind(&mut self);
    fn num_stages(&self) -> usize;            // getNumStages, for label offsets
}
pub trait SlotAlloc<'a> {                     // implemented by core's ArenaAlloc
    fn make<T: Copy + 'static>(&'a self, v: T) -> &'a T;
    fn alloc_scratch_init(&'a self, bytes: usize, align: usize, init: &[u8]) -> MemPtr;
}
pub trait Callbacks<'a, P: StageSink<'a>> {  // SkSL::RP::Callbacks
    fn append_shader(&mut self, p: &mut P, index: i32) -> bool;
    fn append_color_filter(&mut self, p: &mut P, index: i32) -> bool;
    fn append_blender(&mut self, p: &mut P, index: i32) -> bool;
    fn to_linear_srgb(&mut self, p: &mut P, color: MemPtr);
    fn from_linear_srgb(&mut self, p: &mut P, color: MemPtr);
}
```

In C++, `Program::appendStages` and the callbacks hold the same `SkRasterPipeline*`. Here the sink
is passed into each callback, so there is only one `&mut` at a time. The branch fix-up
(label → absolute stage index → relative offset) and `resetBasePointer` after every `invoke_*`
are ported as written. `ProgramDesc` is already highp-only for SkSL programs: no SkSL op has a lowp
version.

### 6.4 Dumper and tracing

`Program::dump` (`#L2490-L3850`) works from the `ProgramStage` list and the slot layout. It needs
the same slot regions Skia derives from pointers (values `v`, temp stack `$`, immutable `i`, uniforms
`u`), so `make_stages` records the regions in the dump's own context. `DebugTracePriv`'s data
(slot/function info, source lines) lands with the dumper (S16). The JSON trace format
(`tools/sksltrace/SkSLTraceUtils.cpp`, test tooling built on `SkJSONWriter` and
`modules/jsonreader`), `DebugTracePlayer` and the `TraceHook` implementation come in S23. The `TraceHook` trait in simd stays where it is; sksl implements it
(`&self` methods, so the recording hook uses a `Mutex<Vec<_>>`).

### 6.5 As implemented in S15

`crates/skia-rust-sksl/src/codegen/rp/append.rs` ports `Program::allocateSlotData` and
`Program::appendStages`. `make_stages` (S14, `program.rs`) is unchanged and still returns its own
`Stage { op, ctx: StageCtx }` list; `appendStages` lowers each entry to a simd `Stage<'a>`.

- **Traits.** `StageSink<'a>` (`append`, `append_stack_rewind`, `num_stages`, and two additions:
  `replace_stage`, because a branch's target is known only after its label is placed, and
  `set_lane_count`), `SlotAlloc<'a>` (`make`, `alloc_scratch_init`) and `Callbacks<'a, P>`, taken
  as `Option<&mut dyn Callbacks<'a, P>>`. Core implements `StageSink` for `RasterPipeline<'a>` and
  `SlotAlloc` for `ArenaAlloc`.
- **Lane count and R8.** The stages use `selection().tier.highp_stride()` at append time. The
  pipeline records it (`RasterPipeline::lane_count`), and `run`/`compile` debug-assert that the
  tier matches.
- **The slab.** One `alloc_scratch_init` reserves values, temp stacks, immutable slots and the
  uniform block, in that order, with the immutable values and the uniform bit patterns written in
  its initial image. `ArenaAlloc::scratch_buffer` gives the bytes that the code binding
  `SHADER_SCRATCH` passes to `MemView::write` (the blitter, and the default `on_filter_color4f`).
  Uniform offsets (`Addr::Uniform`) become slab offsets, so the uniform copies read scalars from
  slot memory, as Skia's `const int32_t*` sources do.
- **Contexts are `'static`.** The simd contexts `UniformCtx` and `CopyIndirectUniformCtx` hold
  `src: MemPtr` (not `&[i32]`), so they can be allocated in the arena like every other context and
  `Stage` stays two words (`stages_are_small`). The rp-diff replayer binds each uniform stage's
  values to its own slot (`UNIFORM_SLOT_BASE + stage index`). Two ported `SkRasterPipelineTest`
  cases (`CopyUniforms`, `CopyFromIndirectUniformUnmasked`) bind their uniform arrays as memory
  slots, as Skia's pointer does; their assertions are unchanged.
- **Trace ops are rejected.** `appendStages` returns `false` before appending anything when the
  program has a trace op. Their contexts hold a `&dyn TraceHook`, and S23 provides the hook.
- **Tests.** `raster_pipeline/sksl_tests.rs` builds programs with the builder and runs them on every
  tier (`oracle_selection`): constants and n-way arithmetic, immutable and uniform data, a forward
  jump, a stack-top branch whose target is patched in, a child invocation through `Callbacks`, the
  rejected trace op, and a lane-count mismatch (should panic). The rp-diff uniform cases still
  match their committed expectations.

## 7. RuntimeEffect integration (core)

- **API**: skia-safe's `effects/runtime_effect.rs`: `RuntimeEffect::make_for_{shader, color_filter,
  blender}(sksl, Option<&Options>) -> Result<RuntimeEffect, String>`, `make_shader(uniforms:
  impl Into<Data>, children: &[ChildPtr], local_matrix)`, `make_color_filter`, `make_blender`,
  `uniforms()`, `children()`, `find_uniform`, `find_child`, `allow_*`,
  `RuntimeShaderBuilder::{new, make_shader, set_uniform_float, set_uniform_int, set_child_*}`
  (+ the color filter and blender builders). Private hooks (`SkRuntimeEffectPriv`: stable keys,
  `AllowPrivateAccess`, `ES3Options`, `VarAsUniform`, `UniformsAsSpan`, `TransformUniforms`) go in
  `runtime_effect_priv`, `#[doc(hidden)]`.
- `RuntimeEffect` is a cheap-clone `Arc` handle (like `Shader`). It holds the base `Program`, the
  reflected uniforms/children/sample usages, flags, and `rp_program: OnceLock<Option<rp::Program>>`
  (Skia's `SkOnce` in `getRPProgram`).
- `RuntimeShader` implements `ShaderBase::append_stages(&self, rec: &mut StageRec, m_rec)` as
  `SkRuntimeShader.cpp#L80-L100`: `CanDraw` (ES2 on raster), `m_rec.apply(rec)`,
  `UniformsAsSpan(…, rec.dst_cs, rec.alloc)`, then `program.append_stages(rec.pipeline, rec.alloc,
  &mut RuntimeEffectRPCallbacks { … }, uniforms)`. The callbacks port `SkRuntimeEffect.cpp#L310-L374`:
  a child shader gets its `MatrixRec` marked invalid unless sampled pass-through, a null shader
  appends transparent black, a null blender appends `srcover`, and color-space intrinsics wrap
  `SkColorSpaceXformSteps::apply` in two `exchange_src`. `ColorFilterBase`/`BlenderBase` follow
  the same pattern. `RuntimeColorFilter::filter_color4f` goes through the existing
  `ColorFilterBase` default (an RP run on one pixel).
- `StageRec` needs no new field: the lane count comes from `skia_rust_simd::selection()`
  (decision 7), and slot memory goes through `rec.alloc`.
- **`KnownRuntimeEffects`**: `get_known_runtime_effect(StableKey) -> &'static RuntimeEffect`, one
  `OnceLock` per key (Skia: `SkNoDestructor` statics, `SkKnownRuntimeEffects.cpp#L470-L604`). The
  same sanctioned cache exception applies. `StableKey` keeps Skia's discriminants
  (`kSkiaKnownRuntimeEffectsStart = 500`), which serialization uses.
- `SkMakeCachedRuntimeEffect`'s global LRU cache is a performance device: port it only when a test
  depends on it (Q4).
- Mesh: `MeshSpecification::make` compiles `kMeshVertex`/`kMeshFragment` programs. CPU drawing
  stays a no-op as in Skia (S27).

## 8. Known runtime effects: what each one unblocks

`SkKnownRuntimeEffects.h#L42-L96` lists 31 stable keys. The CPU callers, and the manifest entries
they block, are:

| Effect(s) | CPU caller | Blocked entries | Wave |
|---|---|---|---|
| `Luma` | `SkLumaColorFilter::Make` | `lumafilter` 2 GMs, `composecolorfilter` 2 (also a user runtime color filter) | S20 |
| `HighContrast` | `SkHighContrastFilter::Make` (effects) | `highcontrastfilter` GM, `HighContrastFilterTest` 2 | S20 |
| `Overdraw` | `SkOverdrawColorFilter` | `overdrawcolorfilter` GM, `overdrawcanvas` 2 (file scan) | S20 |
| `Lerp` | `SkColorFilters::Lerp` | `mixercolorfilter` (file scan) | S20 |
| `Arithmetic` | `SkBlenders::Arithmetic`, `SkImageFilters::Arithmetic` | `arithmode` 2 GMs, `xfermodeimagefilter`, `imagefiltersgraph`, `ArithmeticImageFilterBounds` | S20 (+ IF) |
| `Blend` | `SkShaders::Blend(blender, …)` with a non-mode blender, e.g. `drawVertices` (`draw_vertices.rs` already notes it) | some `vertices` GMs (file scan) | S20 |
| `1DBlur{4..28}`, `2DBlur{4..28}` | `SkShaderBlurAlgorithm`: the raster blur engine uses it for every layer color type except A8/RGBA8/BGRA8, i.e. the **f16** config | the f16 result of every blur/drop-shadow image filter GM: up to ~108 GM entries (file scan: `crop_imagefilter` 16, `blurs` 7, `imagefilters*`, `backdrop*`, …) | S21 |
| `Decal` | `FilterResult` with `kRequiresDecalInLayerSpace` | decal-tiled image filter GMs (count only by running them) | S21 |
| `LinearMorphology`, `SparseMorphology` | morphology filter | `MorphologyGM`, `ImageFilter`, `ImageFilterCanComputeFastBounds`, `ImageFilterDilateThenBlurBounds`, `MorphologyFilterRadiusWithMirrorCTM` | S22 |
| `MatrixConv{Uniforms,TexSm,TexLg}` | matrix convolution filter | `matrixconvolution` 6 GMs, 4 `ImageFilterMatrixConvolution*` tests | S22 |
| `Displacement` | displacement map filter | `displacement` GM, 2 tests | S22 |
| `Normal`, `Lighting` | lighting filters | `lighting` GM | S22 |
| `Magnifier` | magnifier filter | `imagemagnifier` 3 GMs, `ImageFilter_UnboundedInputMagnifier_EdgeLeak` | S22 |

All of them run as `kPrivateRuntimeShader`/`ColorFilter`/`Blender` with `AllowPrivateAccess`. They
call `sksl_rt_shader` helpers (`sk_luma`, `sk_decal`, …), so the `rt_shader` module must load
first. The blur effects loop over uniform arrays with a constant bound (`for (int i = 0; i <
kMaxLoopLimit; ++i)`), which exercises ES2 loop unrolling analysis and indirect uniform copies:
that is why S21 lands after S17, not with S20.

## 9. Test harness and WGSL

- **`tests/src/tools/skslc.rs`** ports the parts of `tools/skslc/Main.cpp` the in-scope outputs
  use: pragma parsing (`detect_shader_settings`, `Main.cpp#L349-L490`), program kind by extension, the `.skrp`, `.stage`
  and `.wgsl` writers and `emitCompileError`. **`tests/src/tools/sksl_minify.rs`** ports
  `tools/sksl-minify`. Inputs come from `third_party/skia/resources/sksl`, expectations from
  `third_party/skia/tests/sksl`. Output→input mapping comes from parsing `gn/sksl_tests.gni`, which
  also says whether `--settings` applies.
- **Verification.** A binary `sksl-golden-verify` (sibling of `gm-verify`) prints
  `test <manifest id> ... ok|FAILED` for every in-scope `sksl-golden` entry. A mismatch writes a
  unified diff to `target/sksl-diffs/`. `cargo xtask inventory verify` learns the `sksl-golden`
  kind and runs it. The files are present wherever the Skia checkout is (locally and in the CI
  `inventory` job), and the binary skips with a note when they are absent, following the
  `skip_missing_resource!` pattern.
- `SkSLTest`'s 252 entries become one `def_test!` per `SKSL_TEST`, running `_CPU` (if flagged),
  `_RP` and `_Clone` in that order. `RasterPipelineBuilderTest`'s expected dumps are string
  literals in the test, as in C++.
- **WGSL (Phase 6 scope, CPU-testable).** S26 ports `SkSLWGSLCodeGenerator.cpp` (and
  `SkSLMemoryLayout`'s WGSL rules, which S6 already brings). `ValidateWGSL` uses Tint in Skia. Two
  goldens are `Tint compilation failed.` (R6). Every other output is independent of validation:
  we run naga's validator only in tests, never to change output. Graphite's later needs (the
  `sksl_graphite_*` modules, `ShaderCodeDictionary`'s use of the pipeline-stage generator,
  precompiled runtime effects) are covered by keeping the module loader, program kinds and
  `PipelineStage::ConvertProgram` complete now.

## 10. Work breakdown

Sizes: S < 500 lines, M 500–1,500, L > 1,500 (Rust, excluding tests and generated data). "Unlocks"
counts manifest entries. Each task is one `port/sksl-<name>` branch and PR. Model hints follow
PLAN §8.3: **(O)** = Opus from the start (a design-setting piece), **(S)** = start on Sonnet (large
and stateful), unmarked = Haiku first.

### Wave S-0: bookkeeping and scaffolding (all parallel)

| ID | Task | Sources | Depends | Size | Unlocks |
|---|---|---|---|---|---|
| S0 | Manifest: reclassify `errors/*.glsl` (338) as `todo`; exclude 3 testbeds; `module = "sksl"` on `RasterPipelineBuilderTest` (26) | manifest | — | S | honest denominators |
| S1 | `skia-rust-base`: move `math`, `floating_point`, `scalar`, `t_fits_in`, `to`, `safe_math`, `matrix_invert`, `checksum`, `half` from core; re-export in core | (moves) | — | S | — |
| S2 | `skia-rust-sksl` crate: `Flavor`/`ModuleSource`, embedded module texts + `xtask sksl sync-modules` + equality test, `skstd::to_string`/`%g`, `stod`/`stoi`, `SkSL::String::printf`, `thash` (`SkTHashTable` + `SkGoodHash`), `OutputStream`/`StringStream` | `SkSLString.cpp`, `SkSLOutputStream.*`, `SkTHash.h`, `SkChecksum.h` (Mix) | S1 | M | — |
| S3 | Golden harness: `skslc` + output→input mapping + `sksl-golden-verify` + xtask `sksl-golden` verify kind | `tools/skslc/Main.cpp`, `gn/sksl_tests.gni` | S2 | M | infrastructure |

### Wave S-A: front end (S5 first; then S4, S6–S10 in parallel; S11–S13 close it)

| ID | Task | Skia sources | Depends | Size | Unlocks |
|---|---|---|---|---|---|
| S4 | Lexer from the generated tables (`xtask sksl gen-lexer` transcribes `SkSLLexer.cpp`'s arrays; tested against the C++ on every token class), `Token`, `Position`, `Operator` (+ precedence, `tightOperatorName`) | `SkSLLexer.{h,cpp}`, `SkSLPosition.*`, `SkSLOperator.*` | S2 | M (+ data) | — |
| S5 **(O)** | IR core: pools/ids/layering (§4), all node enums, `Context`, `ErrorReporter` + `Compiler::handleError`/`errorText`, `Mangler`, `ProgramVisitor`/`ProgramWriter`, `clone`, `description` for every node, `Program`/`Module` containers, `ProgramKind`/`ProgramSettings`/`ProgramConfig` | `ir/SkSL{IRNode,Expression,Statement,ProgramElement,Symbol,Program}.*`, `SkSLContext.*`, `SkSLErrorReporter.*`, `SkSLMangler.*`, `analysis/SkSLProgramVisitor.h`, `transform/SkSLProgramWriter.h`, `SkSLProgramSettings.h`, `SkSLCompiler.cpp#L441-L537` | S2 | L | — |
| S6 | Types and symbols: `Type` (all kinds, coercion costs, `checkForOutOfRangeLiteral`, …), `BuiltinTypes` (fixed ids), `SymbolTable` (incl. `addArrayDimension`, `wouldShadowSymbolsFrom`), `Layout`, `ModifierFlags`, `Variable`, `ShaderCaps`/`ShaderCapsFactory`, `MemoryLayout` | `ir/SkSLType.cpp`, `SkSLBuiltinTypes.*`, `ir/SkSLSymbolTable.*`, `ir/SkSLLayout.cpp`, `ir/SkSLModifierFlags.cpp`, `ir/SkSLVariable.cpp`, `SkSLUtil.*`, `SkSLMemoryLayout.h` | S5 | L | `SkSLTypeTest` (1), `SkSLMemoryLayoutTest` (8) |
| S7a | Constructors and literals: `Constructor::Convert` and every `Constructor*`, `Literal` | `ir/SkSLConstructor*.cpp`, `ir/SkSLLiteral.*` | S6 | M | — |
| S7b | Operator expressions: Binary, Prefix, Postfix, Ternary, Index, Swizzle, FieldAccess, VariableReference, TypeReference, FunctionReference, Setting, Poison, ChildCall, EmptyExpression | `ir/SkSL{Binary,Prefix,Postfix,Ternary,Index,Swizzle,FieldAccess,…}Expression.cpp` etc. | S6 | L | — |
| S7c | Functions: `FunctionDeclaration` (main-signature rules per kind), `FunctionDefinition` (finalization of returns), `FunctionCall` (overload resolution, intrinsic constant evaluation), `IntrinsicList` | `ir/SkSLFunction{Declaration,Definition,Call}.cpp`, `SkSLIntrinsicList.*` | S6 | L | — |
| S7d | Statements and declarations: Block, If, For (ES2 loop rules), Do, Switch (+ `SwitchCase`, static switch), Return/Break/Continue/Discard, ExpressionStatement, `VarDeclaration`/`GlobalVarDeclaration`, InterfaceBlock, StructDefinition, Extension, ModifiersDeclaration | `ir/SkSL{Block,IfStatement,ForStatement,DoStatement,SwitchStatement,VarDeclarations,InterfaceBlock,…}.cpp` | S6 | L | — |
| S8 | `ConstantFolder` | `SkSLConstantFolder.*` | S6 | M | — |
| S9a | Analysis I: `ProgramUsage`, `IsConstantExpression`, `IsTrivialExpression`, `IsSameExpressionTree`, `HasSideEffects`, `IsDynamicallyUniformExpression`, `ReturnsInputAlpha`, sample usage, `SkSLAnalysis.cpp` helpers | `analysis/*`, `SkSLAnalysis.cpp`, `SkSLSampleUsage.cpp` | S5 | M | — |
| S9b | Analysis II: `FinalizationChecks`, `CheckProgramStructure`, `GetLoopUnrollInfo`, `GetLoopControlFlowInfo`, `CanExitWithoutReturningValue`, `ReturnComplexity`, `SwitchCaseContainsExit`, `Specialization`, `ValidateIndexingForES2`, `SymbolTableStackBuilder` | `analysis/*` | S5 | M | — |
| S10 **(S)** | Parser (all declarations, layout parsing, statements, expressions, `#version`, `$`-private names in modules) | `SkSLParser.*` | S4, S7a–d | L | — |
| S11 | Compiler driver + `ModuleLoader` (both sources, all kinds) + `convertProgram`/`compileModule`/`finalize`/`FinalizeSettings`; transforms that finalization needs (`FindAndDeclareBuiltin{Functions,Variables,Structs}`) | `SkSLCompiler.cpp`, `SkSLModuleLoader.*`, `SkSLModule.*`, `transform/SkSLFindAndDeclare*` | S7a–d, S8, S9a–b, S10 | M | **114**: 85 `.skrp` + 5 `.wgsl` + 7 `.stage` failure goldens, `SkSLErrorTest` (8), (+S6's 9); **+338** `errors/*.glsl` after S0 |
| S12 **(S)** | Inliner | `SkSLInliner.*` | S11 | L | (part of every optimized output) |
| S13 | Optimizer transforms: `EliminateUnreachableCode`, `EliminateDeadFunctions`/`LocalVariables`/`GlobalVariables`, `EliminateEmptyStatements`, `EliminateUnnecessaryBraces`, `ReplaceConstVarsWithLiterals`, `ReplaceSplatCastsWithSwizzles`, `RenamePrivateSymbols`, `RewriteIndexedSwizzle`, `HoistSwitchVarDeclarationsAtTopLevel`, `AddConstToVarModifiers`; `Compiler::optimize`, `optimizeModuleAfterLoading`, `optimizeModuleBeforeMinifying` | `transform/*`, `SkSLCompiler.cpp#L253-L405` | S11 | M | — |

### Wave S-B: Raster Pipeline back end (S14 and S16 can start right after S2)

| ID | Task | Skia sources | Depends | Size | Unlocks |
|---|---|---|---|---|---|
| S14 | RP `Builder`: instructions, `BuilderOp`/`ProgramOp` (via `rp_ops!`), temp stacks, labels, all peepholes, `finish` | `SkSLRasterPipelineBuilder.h`, `.cpp#L46-L1398` | S2 | M | with S16: `RasterPipelineBuilderTest` (**26**) |
| S15 | `Program::make_stages` / `append_stages` / `allocateSlotData`; `StageSink`/`SlotAlloc`/`Callbacks` traits and core impls; `ArenaAlloc::alloc_scratch_init` + blitter/`filter_color4f` binding of the initial image | `.cpp#L1399-L2489` | S14 | M | (execution for S17) |
| S16 | `Program::Dumper` + `DebugTracePriv` data types (slot/function info, `setSource`) | `.cpp#L2490-L3850`, `tracing/SkSLDebugTracePriv.h` | S14 | M | with S14: the 26 above |
| S17 **(S)** | RP code generator (`Generator`, slot manager, LValues, every statement/expression/intrinsic, child calls, trace ops) | `SkSLRasterPipelineCodeGenerator.cpp` | S13, S15, S16 | L | **280** `.skrp` (259 + 21 unsupported), `RasterPipelineCodeGeneratorTest` (9), `SkSLTest` non-CPU (**69**) |

### Wave S-C: runtime effects on CPU

| ID | Task | Skia sources | Depends | Size | Unlocks |
|---|---|---|---|---|---|
| S18 **(S)** | `RuntimeEffect` (factories, reflection, `MakeSettings`, `MakeInternal` checks, `getRPProgram`, uniforms + `TransformUniforms`, children, `makeShader`), `RuntimeShader`, `RuntimeShaderBuilder`, `runtime_effect_priv`, RP callbacks | `SkRuntimeEffect.cpp`, `SkRuntimeEffectPriv.h`, `SkRuntimeShader.cpp`, `include/effects/SkRuntimeEffect.h` | S17 | L | `SkSLTest` CPU (**183**), most of `SkRuntimeEffectTest` (~30), `SkSLES2ConformanceTest` (2), GMs `runtimeshader` 17, `runtimefunctions`, `runtimeintrinsics` 6, `rippleshadergm`, `kawase_blur_rt`, `destcolor`, `imagedither` (~28) |
| S19 | `RuntimeColorFilter`, `RuntimeBlender`, color filter / blender builders | `SkRuntimeColorFilter.cpp`, `SkRuntimeBlender.cpp` | S18 | M | rest of `SkRuntimeEffectTest`, `RuntimeBlendTest` CPU (1), `runtimecolorfilter` 2, `workingspace` 2, `composecolorfilter` 2 (with S20) |
| S20 | `KnownRuntimeEffects` (all 31 keys' SkSL, `StableKey`, `get_known_runtime_effect`) + simple clients: Luma, HighContrast, Overdraw, Lerp, Arithmetic blender, `SkShaders::Blend(blender)` | `SkKnownRuntimeEffects.cpp`, `SkRuntimeColorFilter.cpp#L140-L181`, `SkHighContrastFilter.cpp`, `SkBlenders.cpp`, `SkBlendShader.cpp#L140-L156` | S19 | M | 10 named (lumafilter 2, highcontrast GM + 2 tests, overdraw GM, arithmode 2, composecolorfilter 2) + ~10 file scan (`overdrawcanvas`, `mixercolorfilter`, `vertices`, `crbug_918512`, …) |
| S21 | Shader blur algorithm (`SkShaderBlurAlgorithm` + `RasterShaderBlurAlgorithm` in the raster blur engine) and the decal effect in `FilterResult` | `SkBlurEngine.cpp#L1274-L1746`, `SkImageFilterTypes.cpp#L1436-L1452` | S20, image-filter port | M | f16 of blur IF GMs (up to ~108, file scan), decal-tiled IF GMs |
| S22 | Hand-off: morphology, matrix convolution, displacement, lighting, magnifier and arithmetic image filters, owned by the image-filter work and listed here as unblocked | `src/effects/imagefilters/*` | S20 (+ S21 for some) | (IF tasks) | 26 entries named on `port/image-filters-3` (incl. `xfermodeimagefilter`, `imagefiltersgraph`, `ArithmeticImageFilterBounds`) |
| S23 | Tracing: `DebugTracePriv`, `DebugTracePlayer`, `TraceHook` impl, `RuntimeEffect::make_traced`; test tooling `SkSLTraceUtils::{WriteTrace, ReadTrace}` (+ `SkJSONWriter`, `SkJSONReader` ports or reuse) | `tracing/*`, `tools/sksltrace/SkSLTraceUtils.cpp`, `src/utils/SkJSONWriter.*`, `modules/jsonreader/SkJSONReader.*` | S16 (DebugTraceTest), S17 + S18 (player, RE traces) | M | `SkSLDebugTraceTest` (5), `SkSLDebugTracePlayerTest` (15), 3 `SkRuntimeEffectTest` trace tests |

### Wave S-D: other generators and tools (parallel with S-B/S-C once S13 lands)

| ID | Task | Skia sources | Depends | Size | Unlocks |
|---|---|---|---|---|---|
| S24 | `sksl-minify` tool port (`compile_module_list`, `optimizeModuleBeforeMinifying(shrinkSymbols)`, element `description()`s, lexer-driven whitespace stripping) + regeneration check of the embedded minified modules | `tools/sksl-minify/SkSLMinify.cpp` | S13 | M | `.minified.sksl` (**77**) |
| S25 | Pipeline-stage generator + `SkShaderUtils::PrettyPrint` | `codegen/SkSLPipelineStageCodeGenerator.cpp`, `src/utils/SkShaderUtils.cpp` (pretty-print part) | S13 | M | `.stage` (**55**), `SkSLPipelineStageTestbed` (1) |
| S26 **(S)** | WGSL generator (feature `wgsl`); naga used only as a test-time validator | `codegen/SkSLWGSLCodeGenerator.cpp` | S13 | L | `.wgsl` (**413**, +2 if R6 resolves), `SkSLWGSLTestbed` (1) |
| S27 | `MeshSpecification::make` (mesh program kinds, attribute/varying validation; CPU draw stays a no-op) | `src/core/SkMesh.cpp` | S18 | M | `mesh` GMs (11) |

### Wave S-E: sweeps

- **S28** GM sweep for runtime effects (`runtime*.cpp`, `workingspace`, `rippleshadergm`,
  `kawase_blur_rt`, `destcolor`, `imagedither`, `vertices` with blenders), one Haiku task per 3–5
  files, after S18/S19.
- **S29** f16 blur and decal re-sweep of the image-filter GMs after S21.
- **S30** Benches: `SkSLBench` (compile times; 7 CPU), `ColorFilterBench` (2).

### Dependency summary

```
S1 ─ S2 ─┬─ S3 ──────────────────────────────────────────────────────────┐ (harness for all goldens)
         ├─ S14 ─┬─ S16 ─────────────── (RasterPipelineBuilderTest 26)      │
         │       └─ S15 ────────────────────────────┐                      │
         ├─ S4 ──────────────────┐                  │                      │
         └─ S5 ─┬─ S6 ─┬─ S7a–d ─┼─ S10 ─┐          │                      │
                │      └─ S8 ────┘       ├─ S11 ─ S12 ─ S13 ─┬─ S17 ─ S18 ─┬─ S19 ─ S20 ─ S21/S22
                └─ S9a, S9b ─────────────┘   (114/452)       │  (280+69+9) (183+…)        S23
                                                             ├─ S24 (77)   S27
                                                             ├─ S25 (55)
                                                             └─ S26 (413)
```

Critical path to the first runtime-effect pixels: S2 → S5 → S6 → S7 → S10 → S11 → S12 → S13 →
S17 → S18. S14–S16 run alongside and are done before S17 needs them. First merged unlocks: S14+S16
(26), S6 (9), then S11 (114, or 452 with S0).

---

## 11. Risks and open questions

| # | Item | Plan |
|---|---|---|
| R1 | **IR shape mistakes are expensive.** Every later task builds on S5/S6. | S5 is Opus. Its PR includes a reviewed sketch of all node types, `clone`/`description` round-trip tests, and a port of `SkSLTest`'s `_Clone` check driven by hand-built IR. |
| R2 | **Hidden order dependence.** A Skia hash container whose iteration leaks into output, ported as `std::HashMap`, gives outputs that differ by platform or by run. | §5 lists the one known case (`THashSet<Slot>`). Every ported `THashMap`/`THashSet` that is iterated gets a comment saying why its order is unobservable, or uses `thash`. Golden runs on all CI platforms catch slips. |
| R3 | **Float text formatting** (`%g` emulation, `0x%08X (…)`, `std::to_string(int)`). | One `skstd` module (S2) with exhaustive tests against values taken from the goldens, plus known `printf` edge cases (exponent thresholds, `-0`, `1e+10`). |
| R4 | **libm in constant folding.** Folding `sin`, `pow`, `exp`, … in `double` uses the platform C library. Goldens came from one platform; MSVC, glibc and macOS can differ by 1 ulp in rare cases, and casting to float usually hides that. | Use `f64` std functions with `// skia-rust: libm`. A golden mismatch traced to this gets a `notes/` entry, not a tolerance. |
| R5 | **Two module variants** can produce different IR, so a GM is right only with the minified modules. | `Flavor::Library` is the default; only the golden harness uses `Standalone`. A test compiles every known runtime effect under both variants and diffs the stage lists (expected: identical; any difference is documented). |
| R6 | **Tint-dependent goldens** (2 `.wgsl` files contain `Tint compilation failed.`). | Treat as `todo` with reason `needs-tint-validation` until we show naga rejects exactly these two. Otherwise exclude them with that reason. Never special-case the file names. |
| R7 | **`errors/*.glsl` reclassification** could include messages that are GLSL-caps-specific. | S11's harness compiles them with `Standalone()` caps as `skslc` did. Any entry whose text needs GLSL-only caps goes back to `excluded` with the specific reason. |
| R8 | **Stride consistency.** SkSL byte offsets come from `selection()` at append time, while the program compiles on the selection at `compile()`. | Record the stride in `RasterPipeline` when SkSL stages are appended, and `debug_assert!` that it matches at `compile`/`run`. Tests that force a tier must force it before building paints (the existing `force_tier` guard scope does that). |
| R9 | **Process-wide caches** (`ModuleLoader`, known effects, optional effect cache). | `OnceLock`, documented as sanctioned memoization like `StrikeCache::global()`. They never change a result. |
| R10 | **Compile and run cost.** Interpreted RP and an un-pooled IR may lose to Skia in `SkSLBench`/GM timing. | Perf is gated per bench (PLAN §9.2). The IR uses `u32` ids and `Vec` pools (cheap). Revisit pooling only with numbers. |
| R11 | **Agent ladder fit.** IR conversion files are stateful and long. | S5 is Opus. S10, S12, S17, S18 and S26 start on Sonnet. The rest are Haiku-sized clusters. The goldens give exact, local feedback (a diff per file), which is the setting where cheaper rungs succeed. |
| R12 | **JSON** for the trace tests (`SkSLTraceUtils`) needs `SkJSONWriter` and `SkJSONReader`, which Skottie and others need too. | S23 ports them (writer in core `utils`, reader with the test tools) or reuses a port if one has landed by then. The JSON text must match Skia's writer exactly. |

Open questions for the coordinator:

1. **Q1** `skia-rust-base` (recommended) or move the nine helper modules into `skia-rust-simd`?
2. **Q2** Reclassify the 338 `errors/*.glsl` goldens as in scope (recommended: they test only the
   front end's error text, which we must match anyway)?
3. **Q3** Schedule S26 (WGSL, 413 entries) during Phase 3 as a parallel wave? (Recommended: yes,
   right after S13. It needs no GPU, and finding IR gaps early is cheaper.)
4. **Q4** Port `SkMakeCachedRuntimeEffect`'s LRU cache now or only when a test needs it?
   (Recommended: only when a test needs it.)

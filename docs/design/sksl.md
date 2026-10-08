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

## 5. Exactness requirements

| Area | Requirement | Where it shows |
|---|---|---|
| Error text | `handleError` formatting (`error: ` + line + `: ` + msg, echo with the 100-char window and `...`, carets over `[start, end)` clamped to the line, `Position` length capped at 255), `errorText()`/`writeErrorCount()` (`N error` / `N errors`), the order in which errors are reported (conversion order, then finalization). Positions are byte offsets; the line is the count of `\n` | `errors/*.glsl` (338), 106 + 7 + 7 failure goldens, `SkSLErrorTest`, `SkRuntimeEffectTest` (`errorText` substrings) |
| Literals | `skstd::to_string(float/double)` = iostream `%g` with precision 7, then 9/17 if the value does not round-trip, then append `.0` when there is no `.`/`e` (`SkSLString.cpp#L23-L56`). Port the `%g` algorithm: Rust has no `%g`, so build it from Rust's correctly rounded `{:.*e}`. `SkSL::stod` = `istream >> double` (correctly rounded; `str::parse::<f64>`), `stoi` = `strtoull(base 0)`, so `010` is octal, plus `u` suffix and `≤ 0xFFFFFFFF` | every generator's output, `.skrp` `0x3F800000 (1.0)` |
| Constant folding | `SKSL_FLOAT` is `double`. Folds happen in `f64` with Skia's range checks and then cast. Intrinsic folds call `std::sin`/`pow`/… in double (`SkSLFunctionCall.cpp`): use the host `f64` functions with `// skia-rust: libm` (R4) | `.skrp`, `.wgsl`, `.minified.sksl` |
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
simd contexts. Offsets are bytes from the slab's `MemPtr`. Uniform data is `&'a [i32]` (borrowed
from the arena, already color-space-transformed by `UniformsAsSpan`). Pointer packing
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

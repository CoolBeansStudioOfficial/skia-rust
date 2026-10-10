# naga and `unrestricted_pointer_parameters` (Graphite gradients with more than 8 stops)

Status: analysis only. Options (a) and (b) do not exist; only (c), a backend-side
transformation, is left, so per the task nothing was implemented. Needs a decision.

## 1. Reproduction (main at 52e997b, wgpu/naga 30.0.1)

`cargo test -p skia-rust-gpu --test wgsl_pipelines every_pipeline` (the corpus; 63 shaders of
2795 pipelines hit it). Dumping the first one:

```
Function { handle: [6], name: "z_Qh4fi",
  source: InvalidArgumentPointerSpace { index: 0, name: "a",
    space: Storage { access: StorageAccess(LOAD) } } }
```

The WGSL (the storage-buffer gradient path, more than 8 stops):

```wgsl
@group(0) @binding(2) var<storage, read> _storage2 : FSStorageBuffer;
fn z_Qh4fi(a: ptr<storage, array<f32>, read>, b: i32) -> vec4<f32> { ... }
fn A_Qh4fiif2(a: ptr<storage, array<f32>, read>, b: i32, c: i32, d: vec2<f32>) -> vec4<f32> { ... }
... A_Qh4fiif2(&(_storage2.fsStorageBuffer), bufferOffset, numStops, t) ...
```

The WGSL parses; the error comes from the validator (`naga/src/valid/function.rs`, the
argument loop accepts only `Private`/`Function` pointers; `valid/type.rs::ptr_space_argument_flag`
agrees).

## 2. What Skia does

`WGSLCodeGenerator::writeFunctionDeclaration` (SkSLWGSLCodeGenerator.cpp ~L1923-1930) writes
`ptr<storage, T, read>` for every unsized-array parameter and the call sites pass `&(...)`. The
generator never emits a `requires` directive (the only `requires` hits are comments and the
reserved-word list). Dawn/Tint accepts it because `unrestricted_pointer_parameters` is a WGSL
language feature Tint enables by default. Our port emits exactly the same text, so (b) does not
apply: there is no divergence to fix.

## 3. What naga 30.0.1 offers

- Front end: `requires unrestricted_pointer_parameters;` is recognised but is
  `UnimplementedLanguageExtension::UnrestrictedPointerParameters`; `front/wgsl/parse/mod.rs` turns
  it into `Error::LanguageExtensionNotYetImplemented`. Adding the directive would make it worse
  (and would change the WGSL anyway).
- Validator: no `ValidationFlags` bit, no `Capabilities` bit; the check is unconditional. wgpu
  30.0.1 exposes no feature or flag for it.
- 30.0.1 is the newest naga/wgpu on crates.io (checked the index); no bump helps.

So (a) does not exist.

## 4. Proposed design for (c) (not implemented)

Constraint: the corpus-compared WGSL (SkSL to WGSL output) must not change. The transformation
happens only where the backend creates the shader module
(`graphite_utils::compile_wgsl_shader_module`), by parsing with naga's front end, rewriting the IR
and handing wgpu `ShaderSource::Naga` (wgpu re-validates, so the output must be valid IR).

Why IR and not text: the front end resolves names and the call graph, and the rewrite is only
semantics preserving when every call passes the same global.

Transformation (monomorphisation of storage-pointer parameters), applied only to modules whose
validation fails with exactly `InvalidArgumentPointerSpace` on a `Storage` pointer (all other
shaders take the existing path untouched):

1. For each function F with a storage-pointer argument P, collect its call sites. Skia's call
   sites pass `&(_storageN.field)`: an `AccessIndex` chain on a `Storage` `GlobalVariable`, or the
   caller's own pointer parameter (propagated). Anything else is rejected with an error, so we
   never miscompile.
2. If all call sites of F resolve to the same global chain, remove P from F and rewrite F's body:
   uses of `FunctionArgument(P)` become the global chain (expression-arena rewrite), and the
   argument is dropped from the `Call` statements.
3. If call sites disagree (two buffers), clone F once per distinct global. The corpus has only one
   storage array (`fsStorageBuffer`), so cloning is a safety net.
4. Process callees bottom-up (WGSL forbids recursion, so the call graph is a DAG).

It would live in `crates/skia-rust-gpu/src/graphite/wgpu/` (e.g. `naga_pointer_args.rs`), be
documented in `gpu.md` §6.3 W4 as the one place the backend does not hand wgpu the WGSL text
unchanged, and be tested on hand-written WGSL, on the real 12-stop gradient WGSL (the transformed
module validates), and by checking that shaders without storage pointers never go through it.

Cost/risk: roughly 300 lines of naga expression-arena rewriting. The result matches what Tint
produces after its own pointer-parameter inlining. Cheaper alternative: keep the tests ignored and
wait for upstream naga to implement the extension; the corpus test already pins
`InvalidArgumentPointerSpace` as the only tolerated failure.

## 5. Consequences if left as is

Still blocked: `TextureFallbackMultiStopGradientsDrawTest` (ignored, failing) and, on
`port/gpu-g14b`, `ThreadedPipelinePrecompileTest` and `ThreadedPipelinePrecompilePurgingTest`.
Nothing was un-ignored; gpu.md §6.3 W4 already describes the limitation accurately.

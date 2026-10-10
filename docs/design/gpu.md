# Design: GPU, Graphite on wgpu (Phase 6)

Status: proposed (2026-10-09). Pin: `chrome/m156` (`95ee33d7ec77`). Audience: every agent porting
Skia's Graphite (`src/gpu/graphite/**`, `src/gpu/*`, `src/gpu/tessellate`, `src/text/gpu`), the
Dawn backend it runs on in the oracle (`src/gpu/graphite/dawn/*`, ported onto wgpu), the Graphite
unit tests and the GPU GM sweeps. PLAN §8.3 assigns this note to Opus ("how Graphite's
Recorder/DrawPass map onto wgpu and Rust ownership").

Skia references are `path#Lx-Ly` in the pinned tree (`third_party/skia`). Dawn references are
`dawn@e03f1d59:path#Lx-Ly`: the Dawn revision Skia's `DEPS#L42` pins, which the oracle built. It
is not in the shared checkout; the files cited were read from `dawn.googlesource.com` at that
revision. wgpu references are `wgpu-hal 30.0.1` / `wgpu-types 30.0.0` (crates.io, the latest
stable release on 2026-10-09). Manifest counts are from `inventory/manifest.toml` at `66820e9`.
Golden facts were computed from `hashes-m156.json` and `meta-m156.json` of release
`goldens-m156` (`inventory/goldens.lock`). The SkSL crate (including the S26 WGSL generator) is on
`port/sksl-all` and is not yet on `main`. This note assumes it lands first.

## Decisions at a glance

1. **The GPU goldens come from Dawn on D3D12 and Vulkan only. No lavapipe golden exists.** Release
   `goldens-m156` has three GPU tiers, each with 910 GM results: `gpu-dawn-d3d12-warp` (gating),
   `gpu-dawn-d3d12-rtx4070s` and `gpu-dawn-vk-rtx4070s` (both report-only). All results are
   `RGBA_8888`, premul, no color space, so they do not depend on N32 byte order. The oracle ran
   `--src gm` only. Graphite unit tests have no goldens; they rely on their own assertions. The
   Linux/lavapipe tier PLAN §5.1 asks for was never built (`oracle/README.md`, open problems), and
   the Windows oracle host no longer exists (§1).
2. **Dawn's caps were not recorded, but the pinned sources determine them.** On D3D12, Skia
   builds Dawn without DXC (`DAWN_USE_BUILT_DXC` is `OFF`), so Dawn compiles with **FXC**,
   `ShaderF16` is never exposed, and every Graphite WGSL shader on that tier is full `f32`. Dawn
   compiles FXC with `OPTIMIZATION_LEVEL0 | IEEE_STRICTNESS | PACK_MATRIX_ROW_MAJOR`, maps
   `depth24plus-stencil8` to `D32_FLOAT_S8X24`, and exposes features wgpu lacks
   (`DawnLoadResolveTexture`, `DawnPartialLoadResolveTexture`, `TextureFormatsTier1`,
   `RenderPassRenderArea`, `DawnAllowUndefinedLoadStoreOp`). The oracle requested all of them
   (`tools/graphite/dawn/GraphiteDawnToggles.cpp#L46-L75`), so Graphite's choices in the goldens
   assume features wgpu cannot provide (§1.3).
3. **Byte-identical WGSL is achievable by construction and testable without a GPU.** Graphite
   builds SkSL text from a `PaintParamsKey` and a `RenderStep` (`ShaderInfo`), compiles it with
   SkSL's WGSL generator (done as S26), and hands the WGSL to Dawn. Everything up to and including
   the WGSL is deterministic CPU code that we port function by function. We verify it at three
   levels: the 420 `tests/sksl/**/*.wgsl` goldens (416 already pass), the set of pipelines each GM
   creates, and the precompile combinations. The second and third need a dump of Skia's own
   pipelines (decision 8) (§6).
4. **Byte-identical pixels are not guaranteed by construction. They depend on the shader compiler
   below the WGSL.** Dawn compiles WGSL with Tint and wgpu compiles it with naga. On D3D12 the two
   emit different HLSL, and wgpu 30 calls FXC with different flags (`ENABLE_STRICTNESS`, default
   O1, no IEEE strictness) (`wgpu-hal src/dx12/shader_compilation.rs#L232-L240`). FXC at O1 is
   free to fuse and reorder. So **on WARP, any GM whose shaders do non-trivial float math is
   expected to differ** until wgpu can be told to use Dawn's FXC flags. On Vulkan both compilers
   emit near-literal SPIR-V and the driver does the optimizing, so lavapipe is the backend most
   likely to match. Rasterization, blending, MSAA resolve and sampling happen in the same driver
   on both sides and are expected to agree (§2).
5. **Gate per GM on measured classes, never on expectation.** The m156 goldens already show
   three classes. **148** WARP results are byte-identical to the CPU RGBA golden of the same GM
   (image copies, pixel-aligned fills). **192** are identical across WARP, RTX/D3D12 and
   RTX/Vulkan, so they are insensitive to compiler and hardware. **687** are WARP-specific. Each
   `gm-gpu` entry records its class. A class is assigned only after the GM has been run, never
   predicted (§1.2, §3.3).
6. **Revise PLAN §6.2 and §7 (proposal).** PLAN §6.2 says GPU GMs must be "exact on lavapipe and
   WARP". It changes to: exact on `gpu-dawn-vk-lavapipe` (a new tier, decision 8) for every GM
   whose trace replays exactly; exact on WARP for the GMs that replay exactly there; the rest stay
   `todo` with a written root cause (`naga-hlsl`, `fxc-flags`, …) until it is fixed upstream.
   The environment-invariant class (192) also gates on every adapter CI has.
7. **One crate, `skia-rust-gpu`, with one concrete backend: a port of Graphite's Dawn backend,
   written against wgpu** (wgpu and Dawn implement the same WebGPU API). No backend trait: the
   Metal and Vulkan backends are out of scope, and their tests are already excluded. Module
   layout mirrors Skia's directories (§4).
8. **A CI-hosted GPU oracle is needed (Q1, the main question for the maintainer).** The Windows
   host is gone, so Phase 6 cannot reach its exit criteria without new oracle runs. Proposal
   (task G0b): a Linux container (pinned Mesa lavapipe) and a Windows runner (WARP) build Skia and
   Dawn at the pin and run DM as the server did, with Graphite restricted to features wgpu has
   (the restriction PLAN §5.1 planned and never got). Each run writes four dumps: the caps, every
   pipeline's SkSL and WGSL, a **command trace** (pipelines, passes, buffer and texture bytes,
   draws), and the goldens. The trace feeds `gpu-replay` (decision 9) and the CPU-side
   exactness tests (§3).
9. **`gpu-replay` de-risks the phase before any Graphite code is ported.** It replays the oracle's
   command trace through wgpu on WARP or lavapipe and compares the readback with the golden. That
   answers "can wgpu reproduce this GM at all?" for all 910 GMs up front, separates naga and
   driver problems from porting bugs, and assigns the classes of decision 5. Our port writes the
   same trace format, so `gpu-trace diff` shows the first differing command, the GPU counterpart
   of `rp-dump`/`rp-diff` (§3.2, §3.3).
10. **Ownership:** `Context` is owned and `Send`. `SharedContext` is an `Arc` shared by the
    context, its recorders and the precompile context. `Recorder` is `!Send` (as in skia-safe),
    and its devices hang off it through `Rc`/`Weak`. `Recording` is `Send` and holds only `Arc`
    data. GPU resources are `Arc` cells with Graphite's two ref counts (usage and command buffer)
    made explicit, so the resource cache's return queue and budget tests behave as in C++.
    `DrawPass` command lists are plain `Vec`s. No `unsafe` and no lifetimes on public types (§5).
11. **Pipelines compile asynchronously on an executor.** wgpu has no `createRenderPipelineAsync`,
    so a `PipelineCreationTask` runs `Device::create_render_pipeline` on a worker thread (wgpu
    devices are `Sync` on native), and `GraphicsPipelineHandle` resolves at `prepareResources`, as
    in Graphite. `wgpu::PipelineCache` (Vulkan only) sits below Skia's
    `PersistentPipelineStorage`, which stays a port of Skia's key-level storage (§5.4).
12. **About 170 of the 277 in-scope `module = "gpu"` entries run without a GPU (52) or on wgpu's
    `noop` adapter (about 115).** Geometry, uniform layout, tessellation, keys, the dictionary, `ShaderInfo`, DrawList
    and DrawPass building, ClipStack decisions, atlas packing, precompile enumeration and resource
    cache bookkeeping are CPU code. Tests that read pixels need lavapipe, WARP or Metal (§7).
13. **Work order (§9):** bookkeeping and the oracle (G0) → CPU-only foundations, each unlocking
    unit tests immediately (G1–G4: 49 entries) → keys, dictionary and `ShaderInfo`, which make the
    WGSL criterion testable (G5–G6) → resources, recorder and the wgpu backend, which unlock the
    context tests (G7–G11) → drawing and atlases, which unlock the GM sweeps, ordered by class
    (G12–G18).

---

## 1. What the GPU goldens contain

### 1.1 Tiers and how DM rendered them

| Tier | DM config | Adapter | CPU cap | Results | Role |
|---|---|---|---|---|---|
| `gpu-dawn-d3d12-warp` | `grdawn_d3d12` | Microsoft Basic Render Driver (WARP, D3D12 10.0.26100.9278) | `ml3` | 910 | gating |
| `gpu-dawn-d3d12-rtx4070s` | `grdawn_d3d12` | RTX 4070 SUPER | `ml3` | 910 | report |
| `gpu-dawn-vk-rtx4070s` | `grdawn_vk` | RTX 4070 SUPER | `ml3` | 910 | report |

All three tiers come from one build (`gpu-dawn`: `skia_enable_graphite`, `skia_use_dawn`, D3D12 and
Vulkan on, Ganesh off, clang-cl 23.1.2, `-ffp-contract=off`; `oracle/tiers.toml`). Result ids are
`grdawn_d3d12/gm/<name>` and `grdawn_vk/gm/<name>`. Every `meta.json` entry reads `RGBA_8888`,
`Premul`, color space `none`.

`GraphiteSink::draw` (`dm/DMSrcSink.cpp#L2187-L2254`) makes a **fresh `ContextFactory`, `Context`
and `Recorder` for every GM**. It draws into a surface with `SkSurfaceProps(0,
kRGB_H_SkPixelGeometry)` (`#L2256-L2262`, so LCD text is on), reads it back with
`surface->readPixels`, then snaps, inserts and submits. DM's defaults
(`tools/flags/CommonFlagsConfig.cpp#L281-L287`, `CommonFlagsGraphite.cpp#L31-L54`) apply:
`fExecutor` is a 2-thread pool (`--gpuThreads 2`, so pipelines compile asynchronously),
`fInternalMultisampleCount` = 4, `fMinimumPathSizeForMSAA` = 0. So the path strategy is
`kTessellation` (`RendererProvider.cpp#L97-L106`; Vello and sparse strips are compiled out,
`gn/skia.gni#L35-L37`). The CPU work Graphite does (path masks, glyph masks, uploads, color
conversion) ran at runtime level `ml3`. **Our harness forces the CPU tier of class `x64-3-ml3`
for GPU GMs**, using the model tiers of `docs/design/raster-pipeline.md` §4.6 on hosts without
AVX2.

The oracle's logs show the same 227 "Key context creation failed … draw dropped" and 123 "Path
effect failed to apply" warnings on all three tiers (`oracle/README.md`). Those dropped draws are
Graphite behaviour, and the port must drop the same draws.

### 1.2 Measured classes

Comparison of the 910 WARP results with the other tiers, by GM name:

| Class | Count | Definition | Reading |
|---|---|---|---|
| P0 `cpu-identical` | 148 (145 equal the `ml3` RGBA golden) | WARP bytes equal the CPU `RGBA_8888` golden of the same GM on some RGBA tier | No shader arithmetic reaches the pixels: image copies at integer offsets, nearest sampling, pixel-aligned fills, empty GMs (`3x3bitmaprect`, `bitmap_subset_shader`, `clipped-bitmap-shaders-*`, `encode-*`, `orientation_*`, `giantbitmap_*_point_scale`, …) |
| P1 `env-invariant` | 192 (120 also in P0) | WARP = RTX/D3D12 = RTX/Vulkan | Insensitive to compiler and hardware: `aaclip`, `lcdtext`, `textblob*`, `zero_length_paths_*`, `bigbitmaprect_*`, `colorwheel`, `thin_aa_dash_lines`, … |
| `warp+rtx-d3d12` | 7 | equal on the two D3D12 tiers only | — |
| `warp-only` | 687 | differs from both RTX tiers and from the CPU | depends on rasterizer or shader-compiler details |

P0 ∪ P1 = 220 GMs. Pairwise agreement: WARP = RTX/D3D12 on 199, WARP = RTX/Vulkan on 195,
RTX/D3D12 = RTX/Vulkan on 341. So the hardware matters more than the API, and the shader compiler
(FXC vs SPIR-V) still splits about 160 GMs on identical hardware. 24 GMs exist only on GPU
(`wacky_yuv_formats*`, `yuv420_odd_dim*`, `graphitestart`, `graphite-replay`, `new_texture_image`,
`async_rescale_and_read_yuv420_*`, …) and 23 only on CPU (`custommesh*`, `mesh_*`,
`rrect_clip_*`, `draw_quad_set`, …).

### 1.3 What Dawn did (reconstructed; the oracle recorded no caps)

`GraphiteDawnTestContext` requests every feature on Skia's preferred list
(`tools/graphite/dawn/GraphiteDawnToggles.cpp#L46-L75`) and the adapter's full limits. It sets the
toggles `skip_validation`, `disable_lazy_clear_for_mapped_at_creation_buffer`,
`disable_robustness` and `enable_spirv_validation` (`#L23-L39`). From the pinned Dawn:

| Item | D3D12 (WARP, RTX) | Vulkan (RTX) | Graphite consequence | wgpu 30 |
|---|---|---|---|---|
| Shader compiler | Tint → HLSL → **FXC** (no DXC: `dawn@e03f1d59:CMakeLists.txt#L164` `DAWN_USE_BUILT_DXC OFF`, so `UseDXC` is force-set false, `PhysicalDeviceD3D12.cpp#L655-L672`) | Tint → SPIR-V → driver | — | naga → HLSL → FXC or DXC; naga → SPIR-V |
| FXC flags | `OPTIMIZATION_LEVEL0` (`fxc_optimizations` off), `PACK_MATRIX_ROW_MAJOR`, `IEEE_STRICTNESS` (`RenderPipelineD3D12.cpp#L343-L366`, `#L386-L391`) | — | — | `ENABLE_STRICTNESS` only, default O1 (`dx12/shader_compilation.rs#L232-L240`); not configurable |
| `ShaderF16` | **no** (only with built DXC, `PhysicalDeviceD3D12.cpp#L186-L191`) | yes on NVIDIA (`PhysicalDeviceVk.cpp#L382`) | D3D12: `fForceHighPrecision`, `Layout::kStd140`/`kStd430` (`DawnCaps.cpp#L330-L338`, `DawnGraphicsPipeline.cpp#L343-L345`) | `SHADER_F16` (DX12 needs DXC); **off by default in our caps** (below) |
| Storage buffers | yes (`DawnCaps.cpp#L358-L363`) | **no** (Vulkan excluded there) | SSBO vs UBO paint/step data; different WGSL | yes |
| Immediates | `maxImmediateSize` = 64 (`Constants.h#L58`) | 64 (`PhysicalDeviceVk.cpp#L974`) | intrinsics via `var<immediate>` (`DawnCaps.cpp#L343-L344`) | `IMMEDIATES` |
| Dual-source blending | yes (`PhysicalDeviceD3D12.cpp#L161`) | yes | blend formulas with `@blend_src` | `DUAL_SOURCE_BLENDING` |
| MSAA load from resolve | `DawnLoadResolveTexture` + `Partial…` (`#L177-L178`) | `DawnLoadResolveTexture` (`#L500`) | `ExpandResolveTexture` load op, `kMSAARenderArea` attachments, no emulation (`DawnCaps.cpp#L398-L425`) | **none**: Graphite takes its emulation path (`fEmulateLoadStoreResolve`) |
| Undefined load/store | `DawnAllowUndefinedLoadStoreOp` | same | discard ops | **none**: clear/store |
| Render area | `RenderPassRenderArea` | same | — | **none** |
| `TextureFormatsTier1`, `Unorm16TextureFormats` | yes | yes (hardware) | more renderable/filterable formats, fewer fallbacks | `TEXTURE_FORMAT_16BIT_NORM`; no tier-1 feature |
| `depth24plus-stencil8` | **`D32_FLOAT_S8X24`** (`d3d/UtilsD3D.cpp#L338-L341`) | `D24_UNORM_S8` if supported | painter's depth precision | `D24_UNORM_S8_UINT` (`auxil/dxgi/conv.rs#L67`) |
| MSAA | 4×, `SampleDesc.Quality = 0` | 4× | — | 4×, `Quality = 0` |
| Robustness | disabled by toggle | disabled | — | naga bounds checks on (turning them off is `unsafe`) |

`WgpuCaps` never reports `ShaderF16` for a real device (`CapsProfile::from_device`; opt in with
`from_device_with_f16`), even where the adapter has it. wgpu 30 rejects the `half4` fragment
outputs Graphite's f16 WGSL writes for an `Rgba8Unorm` target (Dawn accepts them), and the D3D12
goldens were rendered without f16, so all-f32 WGSL matches the gating tier. The Dawn Vulkan
oracle profile keeps `SHADER_F16`, and `a_half_precision_fragment_output_is_a_creation_failure_on_wgpu`
pins the wgpu failure.

Two consequences:

- **The existing goldens assume Dawn-only features.** In particular, MSAA load-from-resolve goes
  through Dawn's internal `ExpandResolveTexture`, while on wgpu Graphite emulates it with its own
  blit. Both are exact texel copies, so the pixels are expected to agree, but the pipeline keys,
  attachment sizes and command streams differ. G0b therefore renders a **wgpu-restricted** tier
  (decision 8), and the existing tiers serve as a cross-check: a GM that differs between
  restricted and unrestricted Dawn runs depends on a Dawn-only feature, and its old golden is
  unreachable.
- **D3D12 and Vulkan run different WGSL** (f32 vs f16 types, SSBO vs UBO). Every WGSL test is
  per profile (§6.3).

---

## 2. Can wgpu match Dawn byte for byte?

Each stage between "Graphite records a draw" and "the readback bytes":

| Stage | Same bytes? | Why / plan |
|---|---|---|
| Draw decomposition, sort keys, uniform/vertex/instance bytes, texture uploads, CPU masks | **Yes, by construction** | Ported CPU code under the arithmetic rules. `std::sort` tie order (`DrawList.cpp#L128`) can only reorder draws with equal keys, which compressed painter's order gives only to non-overlapping draws; if a test ever observes it, port MSVC STL's `std::sort` (the oracle used clang-cl with MSVC's STL). Checked against the trace, without a GPU (§3) |
| SkSL text, WGSL text | **Yes, by construction** | §6 |
| WGSL → HLSL/SPIR-V | **No** | Tint and naga are different compilers. SPIR-V from both is near-literal (one `OpFMul`/`OpFAdd`/`OpExtInst` per WGSL op, no `NoContraction` from either), so the driver sees the same arithmetic. HLSL differs more: matrix packing (Tint relies on `PACK_MATRIX_ROW_MAJOR`, naga on its own layout), polyfills (`select`, integer division, `clamp`, packing), temporaries |
| HLSL → DXBC (FXC) | **No, today** | Dawn uses O0 + IEEE strictness, wgpu uses O1 without it. O1 may contract `mul`+`add` into `mad` and reorder; with IEEE strictness FXC may not. This is a known cause of divergence, not a guess. Fix: a wgpu option for FXC flags (upstream PR, task G-U1). Even with equal flags, FXC at O0 follows the HLSL's shape, so the HLSL differences above still matter. `gpu-replay` measures how many GMs they affect |
| Driver JIT (WARP, llvmpipe) | Yes for equal input | Same driver, same input bytecode, same output. Cross-host determinism is measured in G0b: WARP and llvmpipe JIT for the host CPU (FMA, AVX-512) |
| Rasterization, sample positions | Yes | Same driver, same API rules (D3D12/Vulkan top-left, standard 4× pattern at quality 0 on both sides) |
| Fixed-function blending | Yes | Same driver, same blend state (ported from `DawnGraphicsPipeline.cpp`), same attachment format (`RGBA8Unorm`) |
| MSAA resolve | Expected | Dawn on D3D12 resolves through render-pass end or `ResolveSubresource`, depending on toggles; wgpu calls `ResolveSubresource` (`dx12/command.rs#L1095`). On WARP both should reach the same resolve; G0c measures it |
| Load-from-resolve | Expected | Dawn's `ExpandResolveTexture` vs Graphite's emulation blit: both copy texels exactly. Removed as a variable by the restricted tier |
| Depth/stencil | Expected | Painter's depth uses at most 16 bits of order, which both D24 and D32F preserve. We still request `Depth32FloatStencil8` on DX12 to mirror Dawn's DXGI format (recorded in `docs/API_MAPPING.md`) |
| Sampling | Yes | Same sampler descriptors (`DawnSampler.cpp`), same driver filtering |
| Readback | Yes | Buffer copies |

**Decision.** Everything above the shader compiler is matched exactly and tested without a GPU.
Below it, matching is an empirical, per-GM property:

- **lavapipe** (needs the new tier from G0b) is the primary gate. Near-literal SPIR-V on both
  sides makes most GMs likely to match; `gpu-replay` decides.
- **WARP** gates P0, P1 and whatever else replays exactly. The rest wait for G-U1 (FXC flags) and
  for naga HLSL fixes, each found by diffing naga's HLSL against Tint's for the failing pipeline
  (`oracle/tint`, G0d).
- **Metal** (macOS runners) and real hardware: P1 gates where CI has the adapter; everything else
  is report-only.
- A GM that cannot match keeps `status = "todo"` with `reason = "gpu-divergent: <tier>: <cause>"`,
  where the cause comes from a replay diff (`naga-hlsl:<construct>`, `fxc-flags`,
  `dawn-only-feature:<name>`). It is never marked passing and never excluded silently.

Rejected alternatives:

- **Port Tint** (a compiler the size of a large fraction of Dawn) to get Dawn's HLSL and SPIR-V.
  Far out of scope.
- **Ship precompiled Tint output and pass it through** (`Features::PASSTHROUGH_SHADERS`). The API
  is `unsafe`, and a renderer whose GPU output depends on blobs from a C++ tool is not a port. It
  stays a diagnostic option for `gpu-replay` (Q4).
- **Tolerances.** The project has none.

---

## 3. Oracle work (task G0b–G0d)

### 3.1 CI-hosted GPU oracle (G0b; maintainer decision Q1)

The server is gone, so the GPU oracle moves to CI runners, scripted in `xtask oracle` like the
server's:

- **Linux, lavapipe**: a container image with Skia's build deps and a **pinned Mesa** built from a
  release tag (lavapipe's output changes between Mesa and LLVM versions). Build `gpu-dawn-linux`
  (`skia_use_dawn`, Vulkan only, clang, `-ffp-contract=off`) and run DM with `--config grdawn_vk`,
  `SKIA_ORACLE_DAWN_ADAPTER=llvmpipe`, CPU cap `ml3`. To make llvmpipe independent of the host
  CPU, fix its code generation with Mesa's CPU-capability override and vector-width environment
  variables (`GALLIUM_OVERRIDE_CPU_CAPS`, `LP_NATIVE_VECTOR_WIDTH`; check the names and the
  effect on FMA at the pinned Mesa). Measure the result on two runner CPU types.
- **Windows, WARP**: the same `gpu-dawn` build on `windows-2025` (VS Build Tools, clang-cl, as in
  `oracle/README.md`). Re-render `gpu-dawn-d3d12-warp` and compare with the published hashes.
  That answers the open question of cross-host WARP determinism. Record the version of
  `d3d10warp.dll`. If it differs from 10.0.26100.9278, publish the run as a new tier.
- **wgpu-restricted runs** (`…-wgpucaps` tiers) on both: an oracle-patch filter on
  `AddPreferredFeatures`. It keeps `ShaderF16`, `DualSourceBlending`, `TextureCompressionBC`,
  `Unorm16TextureFormats`, `TimestampQuery`, and `BufferMapExtendedUsages` (≈ wgpu
  `MAPPABLE_PRIMARY_BUFFERS`), drops the Dawn-only features of §1.3, and clamps limits to wgpu's.
  This finally implements PLAN §5.1's restriction.
- **Dumps**, added to the oracle patch (test tooling only):
  - `caps.json`: adapter info, enabled features, limits, toggles, and the resulting `DawnCaps`
    fields.
  - `pipelines/`: for each `GraphicsPipelineDesc`, the label, vertex/fragment SkSL, WGSL, and
    the Tint HLSL or SPIR-V Dawn compiled. Written from `DawnGraphicsPipeline::Make`
    (`#L330-L400`) and `DawnComputePipeline`.
  - `trace/<gm>.bin`: the command trace of §3.2, written from `DawnCommandBuffer`,
    `DawnResourceProvider` and `DawnQueueManager`, the only places Graphite calls `wgpu::`.

### 3.2 The command trace

A compact binary (or JSON lines) per GM:

- every resource creation (texture/buffer/sampler descriptors);
- every `writeBuffer`/`writeTexture`/mapped-buffer flush (offset, length, bytes stored once by
  hash);
- every pipeline (label + hashes of the WGSL and the full render-pipeline state);
- every render or compute pass (attachments, load/store ops, clear values, resolve targets) and
  its commands (`setPipeline`, `setBindGroup` + offsets, `setVertexBuffer`, `setIndexBuffer`,
  `setScissorRect`, `setViewport`, `setImmediates`, `setStencilReference`, `draw*`);
- copies;
- the final readback.

Our wgpu backend emits the same records from the same places (the port of `DawnCommandBuffer` and
friends), behind a test-only `trace` feature. `xtask gpu-trace diff <gm>` prints the first
differing record with its context, like `rp-diff`.

**Status (G11c).** The `trace` cargo feature of `skia-rust-gpu` emits the records
(`graphite::wgpu::trace`: `Record`, `TraceSink`, `JsonLinesSink`, `MemorySink`;
`WgpuSharedContext::set_trace_sink`). A record is an operation name with ordered fields, written as
one JSON object per line, and byte payloads (mapped buffer flushes, queue writes, WGSL, read-back
bytes) go to the sink once per FNV-1a hash as blobs. Resources are named by the trace id their
creation record (`create_buffer`, `create_texture`) carries; the pass records are
`begin_render_pass`, `set_pipeline`, `set_bind_group`, `set_vertex_buffer`, `set_index_buffer`,
`set_scissor_rect`, `set_viewport`, `set_immediates`, `set_blend_constant`, `draw`,
`draw_indexed`, `draw_indirect`, `draw_indexed_indirect`, `blit_with_draw` (the emulated MSAA
load and resolve), `end_render_pass`, and the compute and copy equivalents, then `submit`. Not yet
traced: sampler creation and the final readback of a surface (`map_read` records the bytes of any
mapped read buffer). The oracle side of the format is not written, so there is no `gpu-trace
diff` yet.

Committed artifacts stay small: per-tier hash lists (`oracle/gpu/expected/<tier>/<gm>.txt`, one
line per record hash), as `rp-diff/expected` does. Full traces and shader texts go into the
`goldens-m156` release as `gpu-trace-<tier>.tar`, downloaded on mismatch.

### 3.3 `gpu-replay` (G0c)

A Rust tool (`xtask gpu-replay <tier> [--match]`) that executes a trace through wgpu: it creates
the same resources, compiles the trace's WGSL through naga, records the same passes, reads back,
and compares with the golden. It runs on Windows runners (WARP via `force_fallback_adapter` and
`Dx12Compiler::Fxc`) and in the lavapipe container. Its output is the class table
(`oracle/gpu/classes/<tier>.toml`: GM → `exact` or `divergent:<first differing pipeline>`), which
sets the order of the GM sweeps (§9) and the `reason` of GMs that cannot match yet. It is
independent of our Graphite port, so it can run in week one. With Q4 approved, a `--tint`
diagnostic mode feeds the trace's Tint HLSL or SPIR-V through `PASSTHROUGH_SHADERS` and names
naga as the cause when only the naga run differs.

### 3.4 Tint as a Linux tool (G0d)

`oracle/tint/`: build the pinned Dawn's `tint` executable on Linux (CMake, no GPU) and emit HLSL
and SPIR-V for any WGSL. `xtask gpu-shader-diff <pipeline>` puts naga's HLSL next to Tint's.
That is how each `naga-hlsl:<construct>` cause is found, and it produces the evidence for
upstream naga issues.

### 3.5 If Q1 is declined

Without new oracle runs:

- the WGSL criterion is checked only through the `.wgsl` SkSL goldens and naga validation;
- the CPU-side exactness tests have no reference;
- GMs gate only against the existing WARP tier, by pixels alone, on a Windows runner whose WARP
  build may not match the server's;
- no lavapipe gate exists.

PLAN §10's Phase 6 exit criteria ("all Graphite tests and GPU GMs exact on lavapipe + WARP") then
cannot be met as written. The phase would end at "unit tests passing; WARP GMs passing where they
match; everything else reported".

---

## 4. Crates, layering and public API

### 4.1 Crate

`skia-rust-gpu` (PLAN §3.1), above `core`, `raster`, `effects`, `text` and `codec` in the layering.
Dependencies:

- `skia-rust-sksl` with feature `wgsl`;
- `skia-rust-raster` (path and clip masks, upload conversions, glyph masks);
- `wgpu = "=30.x"` (pinned; bumps run the full GPU suite, PLAN §9.4);
- `pollster` (blocking on adapter and map futures).

The facade feature `gpu` turns it on. Not in Miri (wgpu FFI). wasm32: build-only, with wgpu's
WebGPU backend (Q6).

Modules mirror Skia's directories:

| Skia | `skia_rust_gpu::` |
|---|---|
| `src/gpu/*.{h,cpp}` (Blend, BlendFormula, Swizzle, KeyBuilder, ResourceKey, BufferWriter, Rectanizer*, DataUtils, DitherUtils, BlurUtils, GradientBitmap, TiledTextureUtils, Token) | `gpu::*` |
| `src/gpu/tessellate` | `tessellate` |
| `src/gpu/graphite/*` | `graphite::*` (one module per file pair) |
| `src/gpu/graphite/{geom,render,task,text,compute,precompile,sparse_strips}` | `graphite::{geom,render,task,text,compute,precompile,sparse_strips}` |
| `src/gpu/graphite/dawn` | `graphite::wgpu` (the backend; file names keep `dawn_` → `wgpu_` mapping in `// Port of:` links) |
| `src/text/gpu` | `text_gpu` (`sktext::gpu`) |
| `src/gpu/graphite/{mtl,vk}`, `src/gpu/{mtl,vk,android}`, `ganesh` | not ported |

**No backend trait.** Graphite's virtual bases (`SharedContext`, `ResourceProvider`, `Texture`,
`Buffer`, `Sampler`, `CommandBuffer`, `QueueManager`, `GraphicsPipeline`, `ComputePipeline`,
`Caps`) each have exactly one subclass we port (`Dawn*`). Each pair becomes one concrete struct
with the base half and the backend half as sections, and every function keeps its `// Port of:`
link to the half it comes from. Real polymorphism inside Graphite stays a trait or an enum:
`Task` (enum: Draw, RenderPass, Upload, Copy, Compute, ClearBuffers, SynchronizeToCpu),
`RenderStep` (trait; the renderer set is fixed at context creation), `ComputeStep` (trait),
`PathAtlas` (enum: Raster, plus Compute for completeness, not built), `DrawListBase` (enum:
`DrawList`, `DrawListLayer`).

*One exception, `Caps` (done in G6).* `graphite::caps::Caps` stays a trait: it is `Caps.h`'s
public interface, and the key layer, the shader generators and the recorder run against
profile-driven and fake caps in tests with no device. The data types `Caps.h` declares
(`ResourceBindingRequirements`, `AttachmentSizePolicy`, `SkSL::ShaderCaps` with
`default_shader_caps()` for `setDefaultShaderCaps`) live beside it, and `graphite::wgpu::WgpuCaps`
is `DawnCaps` plus the base class's state, so every function still has one `// Port of:` link.
Code that takes caps takes `&dyn Caps` (`TextureProxy::make(caps, …)` replaced the
`max_texture_size` parameter).

### 4.2 Seams into core

- `graphite::Device` implements core's `Device` trait (`crates/skia-rust-core/src/device.rs`). The
  core `Canvas` holds it as `Box<dyn Device>` (§5.2 for how the recorder reaches it).
- `Image_Graphite` and `Image_YUVA_Graphite` implement core's `ImageBase`; `is_texture_backed()`
  becomes true for them. `SpecialImage_Graphite` is the texture flavor of core's `SpecialImage`
  (§5.5).
- Text: core keeps the `Slug` placeholder (`docs/design/text.md` T13). `text_gpu` provides the
  `SubRunContainer`/`Slug` implementation that `Device::draw_glyph_run_list` uses.
- Image filters: `Device::create_image_filtering_backend` returns Graphite's `skif::Backend` port,
  which uses `SkShaderBlurAlgorithm` (ported to core) with GPU draws. Special images and the image
  links are §5.5 and §5.6.

### 4.3 Public API (skia-safe shape, `third_party/rust-skia/skia-safe/src/gpu/graphite*`)

- `gpu::graphite::{Context, Recorder, Recording, RecorderOptions, ContextOptions,
  InsertRecordingInfo, BackendTexture, TextureInfo, …}`;
- `gpu::graphite::surfaces::render_target(&mut Recorder, &ImageInfo, Mipmapped,
  Option<&SurfaceProps>, Option<&str>) -> Option<Surface>`, `wrap_backend_texture`;
- `gpu::graphite::images::{texture_from_image, wrap_texture, subset_texture_from}`;
- `Context::{make_recorder, insert_recording, submit, read_pixels}`.

skia-safe has `graphite::mtl` and `graphite::vk` for context creation. We add
`gpu::graphite::wgpu::make_context(wgpu::Device, wgpu::Queue, &ContextOptions) -> Option<Context>`
(the counterpart of `ContextFactory::MakeDawn`) and `BackendTexture`/`TextureInfo` constructors
from `wgpu::Texture`/`TextureFormat`. These deviations go to `docs/API_MAPPING.md`.

---

## 5. Ownership, threading, pipelines

### 5.1 Objects

| Skia | Rust | Notes |
|---|---|---|
| `Context` (`unique_ptr`, single owner) | `Context`, owned, `Send + !Sync`, `&mut self` methods | owns `QueueManager`, its `ResourceProvider`, `ClientMappedBufferManager` |
| `SharedContext` (`sk_sp`, shared by context, recorders, `PrecompileContext`) | `Arc<SharedContext>` | immutable after creation except `ShaderCodeDictionary` (`RwLock`, where Skia uses `SkSharedMutex`), `GlobalCache` (`Mutex`), `PipelineManager` (`Mutex` + slots) |
| `Recorder` | `Recorder { inner: Rc<RecorderInner> }`, `!Send` | skia-safe documents `Recorder` as not `Send`/`Sync`. `RecorderInner` holds `RefCell`s for the task list, uploads, buffer managers, atlases, the strike cache, tracked devices |
| `Device`/`DrawContext` (`sk_sp`, raw `Recorder*`) | `Rc<RefCell<DeviceCore>>` behind the `Box<dyn Device>` the canvas owns; `Weak<RecorderInner>` back-pointer | the recorder keeps `Vec<Weak<RefCell<DeviceCore>>>` (`fTrackedDevices`) and flushes them on `snap()`. A dropped recorder makes the upgrade fail, which is `abandonRecorder()`'s "draws become no-ops" |
| `Recording` | owned, `Send` | holds `TaskList`, `Arc` proxies, extra resource refs, finished procs; nothing `Rc` |
| `Task` (`sk_sp`, shared by a device's `fLastTask` and the root list) | `Arc<TaskNode>`, internal `Mutex` (uncontended) for state mutated in `prepareResources`/`addCommands` | — |
| `TextureProxy` (`sk_sp`, lazy, can deinstantiate) | `Arc<TextureProxy>` with `Mutex<Option<TextureRef>>` | `Send + Sync`, since images are |
| `Resource` (usage refs + command-buffer refs + cache ref; returns to the cache at zero) | `Arc<ResourceCell<T>>` with explicit atomic `usage_refs`/`command_buffer_refs`; handles `ResourceRef<T>` and `CommandBufferRef<T>` whose `Drop` decrements and, at zero, pushes onto the cache's return queue (`Weak<Mutex<ReturnQueue>>`) | keeps `GraphiteResourceCacheTest`'s counts (purgeable, budgeted, zero-ref) exact; the wgpu object lives inside `T` |
| `DrawPass` (`DrawPassCommands::List` in an arena) | struct owned by its `RenderPassTask`; `Vec<DrawPassCommand>` (enum) | `GraphicsPipelineHandle` stays an index resolved at prepare time |
| `DrawList::SortKey` (holds `const Draw*`) | holds a `u32` index into the list's `Vec<Draw>` | same 128-bit key; same comparison |
| `PaintParamsKey` (span into an arena) | `Box<[u32]>`; the dictionary interns `Arc<[u32]>` → `UniquePaintParamsID(u32)` | — |
| `UniformManager`, `PipelineDataGatherer` | byte writers over `Vec<u8>` with Skia's `Layout`s (std140, std430, ±F16, Metal) | exact offsets and padding (`UniformOffsetCalculatorTest`) |
| `CommandBuffer` | wraps `wgpu::CommandEncoder` plus `Vec<CommandBufferRef<_>>` tracking | — |
| `QueueManager` / `GpuWorkSubmission` | wraps `wgpu::Queue`; completion through `Queue::on_submitted_work_done` + `Device::poll` | finished procs run in submission order |
| `SkExecutor` (`ContextOptions::fExecutor`) | `Option<Arc<dyn Executor>>`; `Executor::add(Box<dyn FnOnce() + Send>)` | test default: a 2-thread pool, as DM's `--gpuThreads 2` |

No public type carries a lifetime. `Surface`/`Canvas` stay `!Send` (core's `Rc`-based
`SurfaceBase`), which matches skia-safe.

### 5.2 Why the recorder can reach the devices without lifetimes

`render_target(&mut Recorder, …)` returns a `Surface` with no borrow, and later `recorder.snap()`
must flush every device that surface drew into. Skia solves this with raw pointers both ways. We
use `Rc` from the canvas to `DeviceCore` and `Weak` from the recorder to `DeviceCore`, so the
recorder never keeps a device alive. A device keeps a `Weak` to `RecorderInner`, so no cycle
exists. Every mutation goes through `RefCell::borrow_mut`. Graphite calls from recorder into
device only at flush points (`flushTrackedDevices`), never in the middle of a device draw, so
re-entrant borrows cannot occur. A debug assertion documents this.

### 5.3 Threading

`Context` and `Recorder` are single-owner as in Skia (`SingleOwner` becomes `&mut self` and
`!Sync`). Concurrency in Graphite is limited to:

- pipeline compilation on the executor (§5.4);
- precompilation (`PrecompileContext`, `ThreadedPrecompileTest`, `PipelineManagerThreaded`);
- recordings built on one thread and inserted on another (`Recording: Send`).

Everything those threads share lives in `Arc<SharedContext>`.

### 5.4 Async pipeline compilation and the pipeline cache

- `PipelineManager` (`PipelineManager.cpp`) is ported as is: `findOrCreateGraphicsPipeline`
  returns a `GraphicsPipelineHandle`; with an executor and `fUseAsyncPipelineCreation`, a
  `PipelineCreationTask` runs on the executor. Our task builds `ShaderInfo`, runs the SkSL → WGSL
  step, then calls `wgpu::Device::create_shader_module` and `create_render_pipeline`, which
  compiles in the calling thread on native wgpu. The slot is `Arc<PipelineSlot { state:
  Mutex<SlotState>, ready: Condvar }>`; `prepareResources` waits where Graphite does.
- Errors: Dawn's scoped error checks become `push_error_scope`/`pop_error_scope` (blocked on with
  `pollster`), routed to `ShaderErrorHandler` as in `DawnErrorChecker.cpp`.
- `PersistentPipelineStorage` stays Skia's (serialized `GraphicsPipelineDesc`s via
  `SerializationUtils`; its tests count loads and stores). Underneath, when the adapter has
  `Features::PIPELINE_CACHE` (Vulkan), one `wgpu::PipelineCache` per context goes into every
  pipeline descriptor. Its blob is exposed through a skia-rust extension,
  `ContextOptions::wgpu_pipeline_cache_data` / `Context::wgpu_pipeline_cache_data()` (recorded in
  `API_MAPPING.md`).
- Compilation order never affects pixels. The command trace records pipelines by key, not by
  creation order.

### 5.5 Backend-polymorphic special images and the image filter backend (Q-A, decided)

**Questions for the maintainer:** none block this; the change has no public API impact. For the
record, two internal core traits lose bounds: `skif::Backend` and `SkBlurEngine`/`Algorithm`
(`image_filter_types::Backend`, `blur_engine::{BlurEngine, BlurAlgorithm}`) are no longer
`Send + Sync`. Neither is part of skia-safe's API.

**Problem.** Core's `SpecialImage` was a raster-only struct holding a `Bitmap`. Skia's
`SkSpecialImage` is abstract, with `SkSpecialImage_Raster` and `skgpu::graphite::SpecialImage`
(`SpecialImage_Graphite.cpp`). Without the Graphite flavor, `Device::drawSpecial`,
`snapSpecial` and `createImageFilteringBackend` could not be ported, so on Graphite every layer
restore (`drawDevice` → `snapSpecial` + `drawSpecial`) and every image filter drew nothing.
Constraints: core must not depend on the GPU crate, no `unsafe`, and the raster path must not
change in behavior or cost.

**Decision.** Graphite's special image only wraps a Graphite-backed `SkImage` (Skia's own TODO
says special images are going away in favor of images plus a subset). Core already has a
backend-polymorphic image: `Image(Arc<dyn ImageBase>)`, whose `ImageBase` the GPU crate implements
(`image_graphite::Image`). So the texture flavor is expressible in core without knowing the GPU
crate:

```rust
pub struct SpecialImage { subset: IRect, backing: Backing, props: SurfaceProps }
enum Backing {
    Raster(Bitmap),   // SkSpecialImage_Raster
    Texture(Image),   // skgpu::graphite::SpecialImage: a texture-backed core Image
}
```

- Every virtual of `SkSpecialImage` becomes a `match` (`backingStoreDimensions`, `asImage`,
  `onMakeBackingStoreSubset`, `asShader`: the raster subclass's for `Raster`, the base class's,
  with `SkImageShader::MakeSubset`, for `Texture`). `SkSpecialImages::AsBitmap` is `None` for
  `Texture`, as in C++.
- `SpecialImage::make_from_texture_image(subset, image, props)` is the backend-independent half of
  `SpecialImage_Graphite`; `skia_rust_gpu::graphite::special_image::make_graphite` is
  `SkSpecialImages::MakeGraphite` (converts the image with the recorder's image provider first).
- The raster flavor keeps the same fields and code paths: one enum discriminant check, no
  allocation, no dynamic dispatch. The raster tests and the raster image filter backend are
  untouched.

`skif::Backend` drops `Send + Sync`: a backend lives for one filter evaluation on its device's
thread, and Graphite's holds the `Rc`-based recorder (weakly, where C++ holds a raw pointer).
`skif::Context` keeps `Arc<dyn Backend>`; the Graphite constructor allows
`clippy::arc_with_non_send_sync` with a comment.

**Graphite side** (all ported from `Device.cpp` and `TextureUtils.cpp`):

- `Device::drawSpecial` (an `EdgeAAQuad` image draw with the given transform),
  `snapSpecial` (flush and `WrapDevice`, or `makeImageCopy` when forced or not texturable),
  `createImageFilteringBackend` (`MakeGraphiteBackend`). `snapSpecialScaled` keeps the
  `SkDevice` default (`None`): Graphite does not override it.
- `graphite::image_filter_backend::GraphiteBackend`: `makeDevice` (budgeted, approx-fit,
  `kDiscard` scratch devices), `makeImage` (`MakeGraphite`), `getCachedBitmap`
  (`RecorderPriv::CreateCachedProxy`), and the blur engine, which is `SkShaderBlurAlgorithm`.
- `SkShaderBlurAlgorithm` is backend independent (it draws runtime-effect shaders into devices the
  backend makes), so it is ported to core (`core::shader_blur_algorithm`: `Compute2DBlurKernel`,
  `Compute2DBlurOffsets`, `Compute1DBlurLinearKernel`, `renderBlur`, `evalBlur1D/2D`, `blur`) as a
  trait whose only required method is `makeDevice`. The raster engine does not use it
  (`RasterShaderBlurAlgorithm` is not wired up), so raster output is unchanged.
- `SkImageFilter_Base::makeImageWithFilter` and both `SkImages::MakeWithFilter` factories (raster
  in `skia_rust_raster::images`, Graphite in `graphite::image_factories`, with
  `Image_Base::makeNonBudgeted`) are ported with it. They return `(Image, IRect, IPoint)` as
  skia-safe's `images::make_with_filter` does.

**Alternatives considered.**

- *A `SpecialImageBacking` trait object in core* (`Arc<dyn …>` implemented by raster and GPU):
  the open-ended form of Skia's virtuals, but the raster flavor would pay an allocation and a
  vtable call, and no backend needs more than "a texture-backed `Image`".
- *A generic `SpecialImage<B>`*: infects `FilterResult`, `Device` and the canvas with a type
  parameter; rejected.
- *Converting through raster* (read back, filter on the CPU, upload): changes results and defeats
  the GPU backend.

**Tests.** Noop adapter (`crates/skia-rust-gpu/tests/special_images.rs`, CI): `snapSpecial`
wraps or copies the target with the right subset; subsets share the texture; `drawSpecial`
records a draw; the backend's devices, `MakeGraphite`, `getCachedBitmap` and the shader blur
(2D and two-pass 1D) produce texture-backed results of the requested size; a restored layer
reaches the root task list. `ImageFilterMakeWithFilter` (raster) and
`ImageFilterMakeWithFilter_Graphite` (noop) are ported and pass. Real adapter, run in CI with the
cfg (`special_image_pixels.rs`, lavapipe in CI and here): a blur image filter (2D and 1D passes; exact solid
center, clear outside, partial premultiplied edge falling off), and a half-alpha layer composited
into its parent. All targets are `RGBA_8888`, so the bytes do not depend on N32 order.

### 5.6 Live image-to-device links (Q-B, decided)

**Questions for the maintainer:** none; the decision has no public API change versus skia-safe
(`Image` stays `Send + Sync`, `Surface` and `Recorder` stay `!Send`). One Skia quirk is ported as
is and flagged: `Image_Base::isDynamic()` returns true only when some, but not all, of several
linked devices were just unlinked (`return emptyCount > 0` after resetting it to 0 when all were
unlinked), so a surface image with one live device is not "dynamic" for
`onMakeSubset`/`makeColorTypeAndColorSpace`.

**Problem.** In Skia, `Image::WrapDevice` links a surface's image to its `Device`
(`fLinkedDevices`, `sk_sp<Device>`), and `Image_Base::notifyInUse` (called when the image is put
into a paint key, copied, or cached) flushes the device's pending work, so the image sees every
draw made to the surface before the image is used. Our `ImageBase` is `Send + Sync`
(`Arc<dyn ImageBase>`), while a Graphite device is `Rc<RefCell<DeviceCore>>` (§5.1), so an image
cannot hold the device. G10d flushed at `as_image()` time instead, so draws made after
`as_image()` were lost to the image (`NotifyInUseTestAsImage` fails).

**Options.**

1. *A non-`Send` Graphite image* (holding `Rc<RefCell<DeviceCore>>`). Requires either dropping
   `Send + Sync` from `ImageBase` (breaks skia-safe's `Image: Send + Sync` for every image,
   including raster ones that are legitimately shared across threads) or a second, non-core image
   type (breaks `Canvas::draw_image(&Image)` and every API that takes an `Image`). Rejected:
   public API break for all backends to serve one.
2. *An `Arc`-based linking token with deferred flush tasks.* The image holds a `Send + Sync`
   token; the device would register "flush me" closures that run later. A closure that captures
   the device is `!Send` again, and deferring the flush past the draw that reads the image
   reorders tasks (the draw would be recorded before the device's work is in the root list),
   which is exactly the ordering bug `NotifyInUseTest` documents.
3. *Recorder-side tracking.* The recorder already holds every registered device
   (`fTrackedDevices`, `Weak`). The image needs a key to find its device there, and a few facts
   about the device that `Device::notifyInUse` reads without touching the device.
4. *Keep flushing at `as_image()`* (status quo): wrong results, rejected.

**Decision: 2 + 3, synchronous.** The image holds an `Arc<DeviceLink>` per linked device
(`image_graphite::DeviceLink`, `Send + Sync`), the device owns the same `Arc`:

| `DeviceLink` field | what `Device::notifyInUse` reads in C++ |
|---|---|
| `device_id` | the `sk_sp<Device>` itself: the recorder finds the live device by this ID |
| `recorder_id` | `fRecorder == recorder` |
| `target: Arc<TextureProxy>` | `isScratchDevice()` (target not instantiated) and the pending-read proxy |
| `abandoned` (set by `abandonRecorder`) | `!fRecorder` (the device is immutable) |
| `dropped` (set when `DeviceCore` drops) | `device->unique()` (only the image would still hold it) |
| `last_task` (mirrors `fLastTask`) | a scratch device's last snapped draw task |

`Image_Base::notifyInUse`, `linkDevice(s)`, `unlinkDevices`, `isDynamic` and `Device::notifyInUse`
are ported against the link. The scratch branch needs only the link (it records
`last_task` as a dependency of the reading draw, or adds it to the root list for a copy). The
non-scratch branch runs on the recorder's thread: it looks the device up in the recorder's tracked
devices by ID (`RecorderPriv::find_tracked_device`) and calls `flushPendingWork` on it right
away, so task order matches C++ exactly (A1, B1, A2, B2 in `NotifyInUseTestAsImage`).

Two borrow rules replace C++'s free aliasing:

- `add_image_to_key` cannot reach the drawing device (the `KeyContext` only reads the
  `DrawContext`), so the key context records the Graphite-backed images it keys and
  `Device::drawGeometry` notifies them right after `toKey`, before the draw is recorded: the
  same point in the draw as Skia's (after the flush-before-draw, before the draw).
- The drawing device is mutably borrowed during the notify. When the linked device is the
  drawing device itself (a surface drawing its own image), it is flushed through that borrow;
  when flushing the linked device triggers `flushTrackedDevices(dependency)` and the drawing
  device has pending reads of it, the drawing device is flushed through the borrow too
  (`flush_tracked_devices_with_dependency_and_current`). Other borrowed devices are the one
  already flushing, as before.

**API impact.** None on the public, skia-safe-shaped API. `Surface::as_image()` no longer flushes;
the image sees later draws, as in Skia. `wrap_device` takes the device's link. Internal additions:
`TrackedDevice::{device_id, as_device_core, is_cell}` (defaulted, so test doubles are unaffected),
`KeyContext::notify_in_use`, `Image::{link_device, link_devices, notify_in_use, unlink_devices,
is_dynamic}`, `make_non_budgeted`. Draws of the image on another recorder do not flush (Skia:
"Draws of the view on another Recorder will always see the texture content dependent on how
Recordings are inserted").

**Tests.** Noop (`special_images.rs`, CI): `as_image()` records nothing; the first draw of the
image flushes A1; after more draws to A, the next draw flushes B1 then A2 (root task counts);
a surface drawing its own image flushes itself; once the surface drops, the image unlinks on its
next use. Real adapter: `special_image_pixels.rs::a_surface_image_sees_draws_made_after_it_was_taken`
and the full `NotifyInUseTest.cpp` port (`NotifyInUseTestAsImage`, `NotifyInUseTestSnapshot` and
the 29 layer blend-mode cases), all passing on lavapipe; the 31 manifest entries are `passing`
(the adapter cfg, section 7 and section 8).

---

## 6. Byte-identical WGSL

### 6.1 How Skia produces it

1. `PaintParams::toKey` (`PaintParams.cpp`) walks shader, color filter, blender, dst-read and
   clip-shader, and calls `KeyHelpers.cpp`'s `AddToKey` for each effect. Every call appends
   snippet IDs and data to a `PaintParamsKeyBuilder` and its uniforms to the
   `PipelineDataGatherer`. Runtime effects register user snippets in the dictionary in
   first-use order (`ShaderCodeDictionary.cpp#L995-L1024`). DM uses a fresh context per GM
   (§1.1), so the IDs restart for every GM.
2. `ShaderCodeDictionary::findOrCreate` interns the key as a `UniquePaintParamsID`.
3. `ShaderInfo::Make` (`ShaderInfo.cpp`) builds the snippet tree. It emits SkSL text: helper
   functions and variables mangled by key index (`get_mangled_name`,
   `ShaderCodeDictionary.cpp#L56`), uniform and SSBO declarations per `Layout`, the
   `RenderStep`'s vertex SkSL and varyings, and the blend and coverage code chosen by
   `BlendFormula` and the caps. It also yields the `BlendInfo` and sampler descriptions.
4. `SkSLToWGSL` (`src/gpu/SkSLToBackend.cpp`) compiles the text as `kGraphiteVertex` or
   `kGraphiteFragment` with `fSharpenTextures = true`, `fForceNoRTFlip = true` and
   `fForceHighPrecision = !supportsHalfPrecision` (`DawnGraphicsPipeline.cpp#L343-L345`), using
   the minified Graphite modules (`sksl_graphite_frag`/`_vert`, `Flavor::Library`, sksl.md
   decision 6).
5. The WGSL generator (S26) prints the program.

Every step is string or integer work, apart from SkSL constant folding (sksl.md R4: host `libm`
in `f64`). Matching it is a porting task with exact text feedback.

### 6.2 What must match beyond "the same code"

- **Caps.** WGSL depends on `supportsHalfPrecision`, storage-buffer support, immediates,
  dual-source blending, `fFBFetchSupport` and the resolve load op (§1.3). The port computes caps
  from the wgpu adapter exactly as `DawnCaps.cpp` does from the Dawn device. For tests, a
  `CapsProfile` overrides them with an oracle profile read from G0b's `caps.json`:
  `dawn-d3d12-warp`, `dawn-vk-lavapipe`, and the `-wgpucaps` variants.
- **Order-dependent IDs:** runtime-effect snippet IDs, uniform-data and texture-data cache indices,
  pipeline indices in a `DrawPass`. These follow insertion order in Skia. Port the containers'
  insertion semantics (`SkTHashMap`, `sksl.md` `thash`) wherever an index is observable.
- **Labels.** Pipeline labels (`ShaderInfo::pipelineLabel`, `RenderPassDesc::toString`) are part
  of the precompile tests' expectations (`ChromePrecompileTest`, `AndroidPrecompileTest`). They
  are ported verbatim.

### 6.3 Tests that verify it (the PLAN's first exit criterion)

| # | Test | Reference | Needs | Runs on |
|---|---|---|---|---|
| W1 | `tests/sksl/**/*.wgsl` (420; 416 passing on `port/sksl-all`; `IsInf`/`IsNan` need Tint, 2 stale) and `SkSLWGSLTestbed` | the goldens in the Skia tree | — | every platform |
| W2 | **Per-GM pipeline set**: run the GM through a headless recorder (no wgpu device; `CapsProfile`), collect (label, vs-SkSL, fs-SkSL, vs-WGSL, fs-WGSL) hashes, compare with the oracle's `pipelines/` for that GM and tier | G0b dumps | G5–G10 | every platform |
| W3 | **Precompile corpus**: enumerate the precompile combinations Skia's precompile tests use (`ChromePrecompileTest` and `AndroidPrecompileTest` label lists, `CombinationBuilderTest`, `PaintParamsKeyTest`'s random keys with a fixed seed) and compare their WGSL hashes with a dump made by the same enumeration in C++ (G0b, `precompile` mode) | G0b dumps | G14 | every platform |
| W4 | **naga accepts every corpus shader**: every W2/W3 WGSL parses and validates in naga (with `diagnostic(off, chromium.unreachable_code)`, which naga 30 keeps as a user-defined rule; `enable f16`; `var<immediate>`; `@blend_src`) | — | W2/W3 | every platform |
| W5 | Ported unit tests that inspect keys and SkSL: `KeyTest` (3), `PaintParamsKeyTest` (2), `CacheKeyTest` (2), `PipelineDataCacheTest` (1), `UserdefinedStableKeyTest` (4), `RTEffectTest` (4) | their own assertions | G5–G6 | noop adapter |

"Byte-identical WGSL for all Skia shaders" is defined as W1 + W2 + W3 + W4 all green, for both
oracle profiles. W2 and W3 hash files are committed under `crates/skia-rust-gpu/tests/data/wgsl/`
with the full texts in the release, as in `rp-diff`.

**Status after G6** (`port/gpu-g6`):

- *Pipeline* (all of `ShaderInfo::Make` and the shader half of `DawnGraphicsPipeline::Make`):
  `graphite::wgpu::pipeline_shaders::make_pipeline_shaders(caps, dict, rte_dict, rp_desc, step,
  paint_id, error_handler)` builds the `SkSL` (`ShaderInfo`) and compiles it to WGSL
  (`gpu::sksl_to_backend::sksl_to_wgsl`, the minified Graphite modules, `fSharpenTextures`,
  `fForceNoRTFlip`, `fForceHighPrecision` as Dawn sets them). It needs a `WgpuCaps`, which
  `CapsProfile` builds without a device.
- W1: unchanged, 416 of 420 (the Graphite modules now load, which no `.wgsl` golden uses).
- W2: `crates/skia-rust-gpu/tests/wgsl_pipelines.rs` runs a corpus (3 profiles, about 70 paints, every
  `RenderStep` of the `RendererProvider`, RGBA8 and A8 targets: 2,545 pipelines by default,
  `WGSL_FULL=1` for about 12,000) and checks that it is deterministic. `WGSL_DUMP_DIR` writes the
  per-profile pipeline dumps (`name`, pipeline label, FNV-1a hashes of the four shaders) and
  `WGSL_ORACLE_DIR` compares them with the oracle's. **Missing: the oracle dump** (the oracle host
  is gone, so G0b cannot produce it) **and the headless recorder** (G10: `Device`, `DrawPass`,
  `ClipStack`), which is what turns "a GM" into its triples of render pass, step and key. Missing
  paints: image/YUV/picture shaders and clips (G10), perlin noise and mesh (their key blocks are
  ported, the paints need `Device`).
- W3: not started. `ChromePrecompileTest`, `AndroidPrecompileTest`, `CombinationBuilderTest` and
  `PaintParamsKeyTest` need the Precompile API (G14: `PaintOptions`, `PrecompileShader` and the
  rest, `UniqueKeyUtils`).
- W4: done for everything W2 makes. naga accepts every shader except the ones that pass a pointer
  to a storage buffer array as a function argument (the 12-stop gradients on a device with storage
  buffers): that is WGSL's `unrestricted_pointer_parameters`, which Tint implements and naga does
  not. The test names the error and requires that it is the only one.
- W5: `KeyTest` (3), `PipelineDataCacheTest` (1) and `RTEffectTest` (4) are ported and pass on the
  noop adapter. `CacheKeyTest` (2) needs `ImageProvider` and `Image_Graphite` (G10), and
  `PaintParamsKeyTest` (2) the Precompile API (G14).

**Status after G7c** (`port/gpu-g7c`): `RendererProvider` holds every renderer of the oracle build
(the sparse-strip renderers excepted, Q5). `AnalyticBlurRenderStep` and `AnalyticRRectBlurRenderStep`
are ported with their geometry (`Geometry::AnalyticBlur`, `AnalyticRRectBlur`), the
`AnalyticBlurMask` circle case (`MakeCircle`), and the canvas and `Device::drawBlurredRRect` path
for blurred ovals, rects and rrects (`SkCanvas::attemptBlurredRRectDraw`). Not ported, because
they build their look-up table with `std::erf` (`CreateIntegralTable`, `AnalyticRRectBlurMask::Make`'s
CDF), which the standard library lacks and the libm decision (PR #87, on hold) would provide:
`AnalyticBlurMask::MakeRect` (rects) and `AnalyticRRectBlurMask::Make` (rounded rects). Until then
those draws fall back to a regular draw, which in Graphite still ignores the mask filter (as before
this step), so their output is unblurred. `BlurPointCircle` covers the circle path only. The core
canvas's `addMaskFilterLayer` fallback is not ported (core has no mask filter auto-layers).

**Status after G10b** (`port/gpu-g10b`): `graphite::clip_stack::ClipStack` is the whole of
`ClipStack.cpp` (element tree, `SaveRecord`s, combine/simplify, `visitClipStackForDraw`,
`updateClipStateForDraw`, `recordDeferredClipDraws`, analytic clips, depth-only clip draws for both
draw lists). The device calls back through `ClipDrawHooks`. `NonMSAAClip` (`AnalyticClip` +
`AtlasClip`) is in `geom::non_msaa_clip`, and `ShadingParams` keys it with `AddAnalyticClip`
(`key_helpers_ii::add_analytic_clip`, including the atlas block and its texture binding). The one
seam is the clip atlas: `ClipAtlasManager` (G12a) is a trait that `visit_clip_stack_for_draw` calls
exactly as the C++ does; the device passes `None`, so every non-analytic element is a depth-only
clip draw until G12a. W2's clip paints and `ClipStackTest`-style checks are headless tests in
`tests/clip_stack.rs`; Skia has no Graphite `ClipStack` unit test in m156 (the `GrClipStackTest`
entries are Ganesh's `GrClipStack`).

An identity local matrix is not elided anywhere in Skia: the gradient factories end with
`makeWithLocalMatrix(lm ? *lm : SkMatrix::I())` and `SkShader::makeWithLocalMatrix` always wraps,
and Graphite's key code for `SkLocalMatrixShader` folds the gradient's unit-space matrix into that
wrapper. The skia-rust gradients match (`LocalMatrix[LinearGradient4+PreAlpha]`), so there is
nothing to fix.

---

## 7. What runs without a GPU

| Group | Examples | How |
|---|---|---|
| Pure CPU (no context) | geometry, `Transform`, `Shape`, `IntersectionTree`, `BoundsManager`, `Swizzle`, `UniformManager`/`UniformOffsetCalculator`, tessellation and `WangsFormula`, `DrawAtlas` packing, sparse-strip CPU stages | plain `#[test]`; **52 in-scope entries** (13 `DEF_TEST`, 40 `DEF_GRAPHITE_TEST`, minus the excluded `VkProtectedContextTest` one) + 9 `WangsFormulaTest` (`module = "core"`) |
| Headless recorder | DrawList sort, DrawPass command building, renderer choice, ClipStack element decisions, uniform/vertex bytes, pipeline sets (W2), the CPU half of the command trace | `CapsProfile` + a recorder without a wgpu device; compared with G0b traces |
| wgpu `noop` adapter (`Backends::NOOP`, `wgpu-types backend.rs#L27-L39`) | resource cache, proxy cache, texture proxies, recorder/recording lifecycle, keys, precompile, storage context, texture fallback; any test that creates resources and never reads pixels | a real `Context` on the noop backend. Each port says in its PR whether the test reads pixels. About 115 candidates by file (§9) |
| Real adapter | anything that reads pixels (`ReadWritePixelsGraphiteTest`, `ImageOriginTest`, `MultisampleTest`, `ComputeTest`, `AtlasTests`, …) and all GPU GMs | lavapipe (Linux), WARP (Windows), Metal (macOS) |

The real-adapter tests are written against `adapter_backend_context` (software adapters first)
and skip, saying so, when the machine has none; setting `SKIA_RUST_REQUIRE_ADAPTER` makes a missing
adapter an error, for the GPU CI jobs. In the cloud container lavapipe is `mesa-vulkan-drivers`
(`apt-get install mesa-vulkan-drivers`; Mesa 25.2 here, llvmpipe on LLVM 20.1). The first pixels
tests (`tests/wgpu_first_pixels.rs`) run there: cleared and rect-filled targets read back exactly,
and a path drawn through the MSAA render pass whose resolve is emulated (wgpu has no
load-from-resolve), read back with `WgpuContext::read_pixels` (`asyncReadTexture` /
`transferPixels` / `finalizeAsyncReadPixels`).

Surfaces and images read back through the context: `WgpuContext::read_surface_pixels` is
`Device::onReadPixels` (snap, insert, `ContextPriv::readPixels`), `read_image_pixels` the same for
an image, and `asyncReadPixels` draws a source that is not copyable, is bottom-left or needs a
transfer function into a copyable texture (`CopyAsDraw`) first. Ported tests that read pixels use
`def_graphite_adapter_test!` (tests/src/lib.rs), and the GPU crate's own pixel tests
(`crates/skia-rust-gpu/tests/`) use `cfg_attr(not(skia_rust_adapter_tests), ignore = …)`. The
cfg is declared in the workspace `Cargo.toml`. Without it they are ignored, so a machine without
an adapter cannot count them as passing. With it, they run; without an adapter they say so and
return, unless `SKIA_RUST_REQUIRE_ADAPTER` is set, which makes a missing adapter a failure.
Run them locally with `RUSTFLAGS="--cfg skia_rust_adapter_tests" cargo test -p skia-rust-tests
--lib` (and `cargo test -p skia-rust-gpu`) on a machine with an adapter.

---

## 8. CI strategy

| Job | Runner | Content | Gates |
|---|---|---|---|
| existing build/test jobs | all five platforms | `skia-rust-gpu` CPU and noop tests (W1–W5, unit tests on noop) | yes |
| `gpu-lavapipe` | Linux x64, **container with pinned Mesa/LLVM** (the image G0b uses) | GPU unit tests; `gm-gpu` entries against `gpu-dawn-vk-lavapipe[-wgpucaps]` | yes, once G0b shows llvmpipe deterministic across runner CPUs; until then report-only |
| `gpu-warp` | `windows-2025` | GPU unit tests; `gm-gpu` entries against the WARP tier (`Dx12Compiler::Fxc`, `force_fallback_adapter`) | yes for GMs classed `exact` on the runner's WARP version; the job prints the `d3d10warp.dll` version and fails if it differs from the golden tier's |
| `gpu-metal` | macOS arm64 | GPU unit tests; P1 GMs | unit tests yes; P1 GMs report until measured |
| wasm32 | Linux | `cargo build --target wasm32-unknown-unknown -p skia-rust-gpu` | yes (build only) |
| Miri | Linux | `skia-rust-gpu` excluded (wgpu FFI) | — |
| oracle (G0b) | Linux container, Windows | Skia + Dawn build and DM runs; manual dispatch per pin bump | n/a |
| RTX report | maintainer's machine | `xtask gpu-report` on D3D12 and Vulkan against the two RTX tiers | never |

**The lavapipe job, as built.** The Linux x64 jobs of `.github/workflows/ci.yml` (`test`,
`test-release` and `inventory`, which runs `xtask inventory verify`) install
`mesa-vulkan-drivers` (lavapipe, from the Ubuntu archive; not yet the pinned Mesa of G0b), append
`--cfg skia_rust_adapter_tests` to `RUSTFLAGS`, and set `SKIA_RUST_REQUIRE_ADAPTER=1`, so the
adapter-gated tests run and a missing adapter fails. The request is `adapter_backend_context`
with `Backends::all()`, which reads no environment variable, so no `WGPU_BACKEND` is set. The
arm64 Linux, Windows and macOS jobs do not get the cfg, so those tests stay ignored there.

`xtask inventory verify` treats the adapter-gated tests as follows. With the cfg, an ignored
adapter-gated test that is `passing` is a regression, since it should have run. Without the cfg
(a plain `cargo xtask inventory verify` on a machine with no adapter), it is listed as "NOT
CHECKABLE on this host" and is neither a regression nor a pass (`verify::check`, the
`ADAPTER_IGNORE_REASON` match on libtest's `ignored, <reason>` line). An adapter-gated test that
fails on lavapipe is `failing` like any other failing test.

---

## 9. Work breakdown

Sizes: S < 500 lines, M 500–1,500, L > 1,500 of Rust (excluding tests and data). C++ sizes in
brackets include headers. Every task is a `port/gpu-<name>` branch and PR. Agents start on Haiku
(PLAN §8.3). Tasks marked **(S)** start on Sonnet and **(O)** on Opus. "Unlocks" counts manifest
entries (`module = "gpu"` unless noted). The whole port is about 90k C++ lines: Graphite core 31k
`.cpp`, render/geom/task/text 7.6k, Dawn 7.4k, precompile 4.3k, `src/gpu` 5.7k, tessellate 3.2k,
`src/text/gpu` 5.9k, sparse strips 5.1k.

### Wave G-0: bookkeeping, oracle, feasibility (parallel)

| ID | Task | Sources | Depends | Size | Unlocks |
|---|---|---|---|---|---|
| G0a | Manifest and harness: a generated `gm-gpu` kind (one entry per GM registration, `module = "gpu"`, id `gm/<file>.cpp::<Name>@gpu`) so CPU and GPU status stay separate; GM harness support for `grdawn_*` result ids and `RGBA_8888` goldens; exclusions `VkProtectedContextTest` (12, Vulkan protected memory) and `AHardwareBufferTest` (1, Android) | `xtask/src/inventory.rs`, `tests/gm/src/{sink,goldens}.rs` | — | M | honest denominators: 297 → 277 in-scope unit/bench entries + ~1,110 `gm-gpu` |
| G0b **(O)** | CI-hosted GPU oracle (§3.1): Linux/lavapipe container + Windows/WARP; restricted tiers; `caps.json`, pipeline and trace dumps; publish | `oracle/patches`, `xtask oracle`, workflows | Q1 | M | lavapipe tier; WARP determinism answer; W2/W3 references |
| G0c **(S)** | `gpu-replay` + `gpu-trace` format + class tables (§3.2–3.3) | — (new tooling) | G0b | M | per-GM classes for 910 GMs × tiers; the go/no-go for WARP and lavapipe |
| G0d | `oracle/tint` + `gpu-shader-diff` (§3.4) | pinned Dawn | G0b | S | root causes; upstream naga issues |
| G-U1 | Upstream wgpu: configurable FXC flags (optimization level, IEEE strictness) on `Dx12BackendOptions`; follow-ups for naga HLSL differences found by G0d | wgpu, naga | G0c evidence | S each | WARP GMs blocked on `fxc-flags` |

### Wave G-A: CPU-only foundations (no wgpu; start immediately, parallel)

| ID | Task | Skia sources | Depends | Size | Unlocks |
|---|---|---|---|---|---|
| G1 | `skia-rust-gpu` crate + `src/gpu` shared code: `Swizzle`, `Blend`/`BlendFormula`, `KeyBuilder`, `ResourceKey`, `BufferWriter`, `RectanizerPow2`/`Skyline`, `DataUtils` (ETC1/BC1 decode), `DitherUtils`, `BlurUtils`, `GradientBitmap`, `TiledTextureUtils`, `Token` | `src/gpu/*` [5.7k] | — | M | `SwizzleTest` (1) |
| G2 | Geometry: `Rect`, `Transform`, `Shape`, `Geometry`, `EdgeAAQuad`, `IntersectionTree`, `BoundsManager` (all four), `DrawOrder`, `NonMSAAClip`, `CoverageMaskShape`, `SubRunData`, analytic blur mask CPU parts | `src/gpu/graphite/geom/*` [3.5k], `DrawOrder.h` | — | M | `RectTest`, `TransformTest`, `ShapeTest`, `IntersectionTreeTest`, `BoundsManagerTest` (5); `BoundsManagerBench` (6, bench) |
| G3 | Uniform layout: `Uniform`, `UniformManager` (all `Layout`s, half packing), `UniformOffsetCalculator`, `PipelineData` (`UniformDataBlock`, `TextureDataBlock`, `PipelineDataGatherer`, caches) | `UniformManager.*`, `Uniform.h`, `PipelineData.h` [1.5k] | G1 | M | `UniformManagerTest` (19), `UniformOffsetCalculatorTest` (15) |
| G4 | Tessellation: `WangsFormula`, `PatchWriter`, `MiddleOutPolygonTriangulator`, `MidpointContourParser`, `StrokeIterator`, `FixedCountBufferUtils`, `Tessellation`, `CullTest`, `AffineMatrix` | `src/gpu/tessellate/*` [3.2k] | G1 | M | `WangsFormulaTest` (9, core); tessellation benches (~10, bench) |

### Wave G-B: keys and shaders (headless; unlock the WGSL criterion)

| ID | Task | Skia sources | Depends | Size | Unlocks |
|---|---|---|---|---|---|
| G5a **(S)** | `BuiltInCodeSnippetID`, `PaintParamsKey` (+builder, `toString`, `dump`), `ShaderCodeDictionary` (snippet table, interning, runtime-effect snippets, `convertRuntimeEffect`), `RuntimeEffectDictionary`, `KeyContext` | `PaintParamsKey.*`, `ShaderCodeDictionary.*`, `KeyContext.*` [3.4k] | G3, sksl S25 (pipeline-stage generator) | L | with G6: W5 tests on a profile |
| G5b | `KeyHelpers` I: solid/gradient/image/YUV/coord-clamp/dither/local-matrix/perlin/picture shader blocks | `KeyHelpers.cpp#L1-L1500` | G5a | L | — |
| G5c | `KeyHelpers` II: color filters, blenders, runtime effects, color-space xform, primitive color, clip shader | `KeyHelpers.cpp#L1500-L2784` | G5a | L | — |
| G5d | `PaintParams` (simplification, dst-read strategy, `toKey`) | `PaintParams.*` [0.8k] | G5b, G5c | M | — |
| G6 **(S)** | `ShaderInfo` (snippet tree → SkSL, varyings, blend/coverage code, labels), `ContextUtils`, `SkSLToBackend` (WGSL only), `CapsProfile` + `Caps` (backend-neutral half) | `ShaderInfo.*`, `ContextUtils.*`, `src/gpu/SkSLToBackend.*`, `Caps.*` [2.7k] | G5a, G7a | L | W5 (`KeyTest` 3, `PaintParamsKeyTest` 2, `CacheKeyTest` 2, `PipelineDataCacheTest` 1, `RTEffectTest` 4 on noop, after G11) |
| G7a | Render steps I + `Renderer`/`RendererProvider`/`DrawWriter`/`DynamicInstancesPatchAllocator`/`CommonDepthStencilSettings`: `AnalyticRRect`, `PerEdgeAAQuad`, `CircularArc`, `CoverBounds`, `CoverageMask`, `Vertices`, `Mesh` | `render/*`, `Renderer.*`, `RendererProvider.*`, `DrawWriter.*` [~3.5k] | G2, G3 | L | — |
| G7b | Render steps II: `TessellateCurves`, `TessellateWedges`, `TessellateStrokes`, `MiddleOutFan` | `render/Tessellate*`, `MiddleOutFan*` | G7a, G4 | M | — |
| G7c | Render steps III: `BitmapText`, `SDFText`, `SDFTextLCD`, `AnalyticBlur`, `AnalyticRRectBlur` (+ `geom/Analytic*BlurMask`) | `render/*Text*`, `render/Analytic*Blur*`, `geom/Analytic*` | G7a | M | — |

### Wave G-C: resources, recorder, wgpu backend (unlock the context tests)

| ID | Task | Skia sources | Depends | Size | Unlocks |
|---|---|---|---|---|---|
| G8 **(O)** | Resource model: `Resource` (ref counts, §5.1), `ResourceCache` (return queue, budgets, purge), `GraphiteResourceKey`, `ScratchResourceManager`, `ProxyCache`, `TextureProxy`/`TextureProxyView`, `TextureInfo`, `TextureFormat` (+`XferFn`), `ResourceProvider` (neutral half) | [7.1k] | G1 | L | with G11: `GraphiteResourceCacheTest` (8), `ProxyCacheTest` (9), `TextureProxyTest` (3), `TextureFormatTest` (1), `CacheBudgetTest` (1) |
| G9a **(S)** | `Recorder`, `Recording`, `TaskList` + tasks (`Draw`, `RenderPass`, `Upload`, `Copy`, `ClearBuffers`, `SynchronizeToCpu`, `Compute`), `UploadBufferManager`, `BufferManager` (draw/static buffers), `ClientMappedBufferManager` | [≈5k] | G8 | L | with G11: `RecorderTest` (3), `GraphiteContextRecorderTest` (4), `BufferManagerTest` (2), `UploadBufferManagerTest` (1), `RecordingOrderTest` (1), `SubmitWithFinishProcTest` (2) |
| G9b | `Context` (insert/submit/readPixels/async rescale-and-read), `QueueManager`, `CommandBuffer` (neutral half), `GlobalCache`, `PipelineManager` + `PipelineCreationTask` + executor (§5.4), `SharedContext`, `StorageContext` | [≈3.8k] | G9a, G6 | L | `StorageContextTest` (8), `PipelineCallbackTest` (5), `PersistentPipelineStorageTest` (1) |
| G10a **(S)** | Drawing core: `DrawContext`, `DrawList`, `DrawListLayer`, `DrawPass`, `Device` I (state, clip plumbing, rect/rrect/quad/image/vertices/mesh draws, `drawGeometry`, layers, `createDevice`) | `Draw*`, `Device.cpp#L1-L1400` [≈5k] | G7a, G9a | L | headless W2 for P0 GMs |
| G10b **(S)** | `ClipStack` (element tree, analytic/depth-only/atlas clips, `visitClipStackForDraw`, `NonMSAAClip`) | `ClipStack.*` [2.5k] | G10a | L | — |
| G10c | `Device` II: paths (strategy, `chooseRenderer`, inverse fills, stroke-and-fill, hairlines), text entry points, `drawSpecial`, image-filter backend | `Device.cpp#L1400-L2631` | G10a, G7b | M | — |
| G10d | Images and surfaces: `Image_Graphite`, `Image_Base_Graphite`, `SpecialImage_Graphite`, `Surface_Graphite`, `TextureUtils`, `ImageFactories`, `ImageProvider` | [≈3k] | G10a | L | with G11: `RecordingSurfacesTest` (11), `ImageProviderTest` (4), `ImageShaderTest` (1), `MutableImagesTest` (1), `DeviceTest` (1), `TextureFallbackTest` (8), `ImageWrapTextureMipmapsTest` (1), `crbug_513836996` (1), `InnerFillTest` (2) |
| G11a **(S)** | wgpu backend I: `WgpuCaps` (port of `DawnCaps`, profiles), format tables (`DawnGraphiteUtils`), `Texture`/`Buffer`/`Sampler`/`BackendTexture`/`TextureInfo`, `ResourceProvider` (backend half), `SharedContext`, `make_context` | `dawn/Dawn{Caps,GraphiteUtils,Texture,Buffer,Sampler,BackendTexture,TextureInfo,ResourceProvider,SharedContext}.*` [≈3.2k] | G8 | L | context creation; with G8/G9: everything marked "with G11" above; `BackendTextureTest` (3), `DawnBackendTextureTest` (2), `UpdateBackendTextureTest` (2), `TextureSizeTest` (1) |
| G11b **(S)** | wgpu backend II: `GraphicsPipeline` (state from `DawnGraphicsPipeline.cpp`, naga compile, noop fragment), `ComputePipeline`, error checker | `dawn/Dawn{GraphicsPipeline,ComputePipeline,ErrorChecker}.*` [≈1.3k] | G11a, G6 | M | — |
| G11c **(S)** | wgpu backend III: `CommandBuffer` (passes, load/resolve emulation, immediates, copies, readback), `QueueManager`, async map; trace emission (`trace` feature) | `dawn/Dawn{CommandBuffer,QueueManager,AsyncWait}.*` [≈1.7k] | G11b, G9b | L | first pixels: `ReadWritePixelsGraphiteTest` (2), `WritePixelsTest` (1), `MultisampleTest` (2), `VerticesPaddingTest` (2) |

### Wave G-D: atlases, text, compute, precompile

| ID | Task | Skia sources | Depends | Size | Unlocks |
|---|---|---|---|---|---|
| G12a | Path atlases: `DrawAtlas`, `AtlasProvider`, `PathAtlas`, `RasterPathAtlas` + `RasterPathUtils` (CPU masks via `skia-rust-raster`), `ClipAtlasManager` | [≈3.9k] | G10c, G11c | L | `DrawAtlasTest` (4), `AtlasOobTest` (1); path GMs |
| G12b **(S)** | GPU text: `src/text/gpu` (`SubRunContainer`, `GlyphVector`, `VertexFiller`, `StrikeCache`, `SDFMaskFilter`, `DistanceFieldAdjustTable`, `SubRunAllocator`, `SlugImpl`, `TextBlobRedrawCoordinator`) + `TextAtlasManager`, `GlyphData`, `TextStrike` | [≈7k] | G7c, G12a, text T14/T15 | L | `DirectMaskLimitTest` (1), `StrikeForGPUTest` (1, text); text GMs |
| G13 | Compute: `ComputeStep`, `DispatchGroup`, `ComputeTask` (no Vello: `skia_enable_vello_shaders = false`) | `compute/{ComputeStep,DispatchGroup}.*`, `task/ComputeTask.*` | G11c | M | `ComputeTest` (17 in scope; 2 Metal ones excluded) |
| G14 | Precompile: `PaintOptions`/`PaintOption`, `Precompile{Shader,ColorFilter,Blender,ImageFilter,MaskFilter,RuntimeEffect}`, `PrecompileContext`, `PublicPrecompile`, `SerializationUtils` | `precompile/*`, `PrecompileContext.cpp`, `PublicPrecompile.cpp`, `SerializationUtils.*` [5.3k] | G6, G9b | L | `CombinationBuilderTest` (1), `PaintParamsKeyTest` (2), `ChromePrecompileTest` (1), `AndroidPrecompileTest` (3), `PrecompileStatsTest` (1), `ThreadedPrecompileTest` (5), `PipelineManagerEarlyExit` (4), `PipelineManagerThreaded` (4), `UserdefinedStableKeyTest` (4); W3 |
| G15 | YUV and promise images: `Image_YUVA_Graphite`, `YUVABackendTextures`, promise-image lazy proxies, `NotifyInUse` hooks | [≈1.2k] | G10d, G11a | M | `GraphitePromiseImageTest` (7), `GraphiteYUVAPromiseImageTest` (7), `ImageOriginTest` (2), `NotifyInUseTest` (31), `BigImageTest` (2) |
| G16 | Tests in other files: `BlurTest`, `CanvasTest`, `F16DrawTest`, `ImageFilterTest` (2), `RuntimeBlendTest`, `ShaderTest`, `SimplifyPaintTest`, `SkColorSpaceXformStepsTest`, `SkRuntimeEffectTest` (2) Graphite halves | — | G10d, G12a | S each | 12 |
| G17 | Sparse strips (Q5): `Flatten`, `Tiler`, `StripGenerator`, `MSAA_LUT`, `AlphaAtlasManager`, `WideTile`/`EndCap` steps | `sparse_strips/*` [5.1k] | G7a | L | 11 CPU (`ClipTest` 2, `FlattenTest` 2, `LUTTest` 1, `OracleTest` 2, `PolylineTest` 2, `TilerTest` 2) + 12 GPU (`AtlasTests` 7, `CoverageTest` 5) |

### Wave G-E: GM sweeps (`gm-gpu`), ordered by class

Each sweep task covers 5–15 GMs whose CPU port already passes (the GM's drawing code is shared).
Before an entry is marked `passing`, the headless trace must match (W2 + buffer bytes) and the
pixels must match on every gating tier where the GM's class is `exact`.

| ID | Sweep | Depends | Unlocks |
|---|---|---|---|
| G18a | P0 `cpu-identical` (148): image draws, copies, pixel-aligned fills | G10a, G10d, G11c | ≈ 148 (minus those whose CPU port is still `todo`) |
| G18b | P1 `env-invariant` (192 − 120 = 72 more): AA clips, hairlines, text blobs, LCD text | G10b, G12a, G12b | ≈ 72 |
| G18c | Shapes and paths (`exact` on lavapipe from G0c) | G10c, G12a | by class table |
| G18d | Shaders, gradients, color filters, blenders, runtime effects | G5b–d | by class table |
| G18e | Image filters, blur, mask filters | G10c, G16 | by class table |
| G18f | Text (bitmap, SDF, LCD, emoji) | G12b | by class table |
| G18g | YUV / GPU-only GMs (24) | G15 | 24 |
| G18h | WARP re-sweep after G-U1 | G-U1 | GMs blocked on `fxc-flags` |
| G19 | Benches: `SkSLBench` `graphite_small`/`graphite_large` (2), `BoundsManagerBench`, tessellation benches; perf gate vs the restricted oracle (PLAN §9.2) | G6, G2, G4 | benches |
| G20 | RTX report tooling (`xtask gpu-report`) | G11c | report |

### Dependency summary

```
G0a                       G0b ─┬─ G0c ── class tables ──────────────┐
                               └─ G0d ── G-U1                       │
G1 ─┬─ G2 ─┐                                                        │
    ├─ G3 ─┼─ G5a ─┬─ G5b ─┐                                        │
    └─ G4 ─┤       └─ G5c ─┴─ G5d                                   │
           └─ G7a ─┬─ G7b      G6 (G5a, G7a) ── W2/W3 (with G0b)    │
                   └─ G7c                                           │
G1 ─ G8 ─ G9a ─ G9b ─┐                                              │
     G8 ─ G11a ─ G11b ─ G11c ─ first pixels                         │
G7a + G9a ─ G10a ─┬─ G10b                                           │
                  ├─ G10c ─ G12a ─ G12b (+ text T14/T15)            │
                  └─ G10d ─ G15, G16                                │
G6 + G9b ─ G14     G11c ─ G13     G7a ─ G17                         │
G10a/G10d/G11c ─ G18a ─ G18b … G18h  ◄──────────────────────────────┘
```

Critical path to the first GPU GM: G1 → G3 → G5a → G6 → G11b → G11c, with G2 → G7a → G10a and
G8 → G9a → G11a alongside. Unit-test yield by wave: G-A 49 (40 `gpu` + 9 `core`); G-B/G-C about
105 (most on the noop adapter); G-D about 130; G-E the `gm-gpu` entries.

---

## 10. Risks and open questions

| # | Item | Plan |
|---|---|---|
| Q1 | **CI-hosted GPU oracle** (G0b): Actions minutes for a Skia+Dawn build on Linux and Windows (≈ 1–2 h per build on 4 vCPUs, cached by pin + GN args), a Mesa container to maintain, and new release assets in `goldens-m156` | Recommended. Without it, see §3.5: the exit criteria can't be met as written |
| Q2 | **Revise PLAN §6.2/§7** to per-GM classes (decision 6) | Recommended; this note is the proposal. PLAN edits land with G0a |
| Q3 | **What counts as passing for a `gm-gpu` entry** if a GM is `exact` on lavapipe but `divergent` on WARP | Proposed: `passing` = exact on every gating tier where the class table says `exact`, and at least one gating tier must say `exact`; the WARP divergence stays visible in the class table and the report |
| Q4 | **`unsafe` for test diagnostics.** `PASSTHROUGH_SHADERS` (Tint blobs in `gpu-replay --tint`) and `create_shader_module_trusted` (Dawn's `disable_robustness`) are `unsafe` wgpu APIs | Proposed: allow them only in `oracle/` tooling (outside the shipped crates), never in `skia-rust-gpu`; decide before G0c needs `--tint` |
| Q5 | **Sparse strips** (23 entries) are compiled out of the oracle build (`skia_enable_sparse_strips = false`) and used by no GM | Keep in scope as the last wave (G17); their tests carry their own assertions. Alternative: exclude as "experimental, off in the default build" |
| Q6 | **wasm32**: wgpu's WebGPU backend types are `!Send`, but `Recording: Send` | Build-only in CI with wgpu's `fragile-send-sync-non-atomic-wasm` feature (`wasm32-unknown-unknown` has no threads); no GPU tests on wasm in Phase 6 |
| R1 | **naga vs Tint HLSL** differences keep WARP GMs divergent even with equal FXC flags | Measured by G0c, explained by G0d, fixed upstream (G-U1); until then `todo` with the cause. lavapipe remains the main gate |
| R2 | **llvmpipe or WARP not deterministic across hosts** (JIT uses host FMA/AVX-512) | G0b measures on two CPU types; pin Mesa/LLVM and override CPU caps; if WARP still varies, WARP gating moves to re-rendered runner-specific tiers |
| R3 | **Dawn-only features change Graphite's path** (load-resolve, render area, undefined ops, tier-1 formats) | Restricted `-wgpucaps` tiers; unrestricted tiers kept for cross-checks |
| R4 | **CPU work at the wrong tier.** GPU goldens embed `ml3` CPU results (masks, conversions) | The GM harness pins the `x64-3-ml3` class (model tier where needed) for GPU runs |
| R5 | **`std::sort` tie order** in `DrawList` (MSVC STL in the oracle) | Equal keys only for non-overlapping draws; the trace diff would show any reorder; port MSVC's introsort if one appears |
| R6 | **`RefCell` re-entrancy** between recorder flushes and device draws | Flushes happen only at Graphite's flush points; debug assertions; G10a adds a test that snaps while a canvas is alive |
| R7 | **wgpu version churn** (API changes every ~3 months) | Pin `=30.x`; bumps are their own PRs and run the GPU suite (PLAN §9.4) |
| R8 | **Scale**: ~90k C++ lines, the largest phase | Split as above; G-A starts now (CPU-only, no oracle needed); W2 gives text-level feedback long before pixels |
| R9 | **Dropped draws** (227 "Key context creation failed") are behaviour that must be reproduced | Port the failure paths in `KeyHelpers`/`PaintParams` verbatim; W2's pipeline sets show missing or extra draws |

No compile-only skeleton is included. Adding `wgpu` to the workspace pulls it into every CI job
(including Miri and wasm32) and into `cargo deny`'s review. That is better done in G1, together
with the first code that uses it.

# Design: Fuzzing (PLAN §6.4, §6.6)

Status: implemented for the first slice (2026-10-11). Pin: `chrome/m156`. Audience: every agent
porting a file from Skia's `fuzz/` (`DEF_FUZZ`), and whoever sets up the nightly fuzz job.

The manifest has 32 `kind = "fuzz"` entries, one per `DEF_FUZZ` registration in `fuzz/*.cpp`.
The first slice made 13 `passing`. The FuzzCanvas slice makes four more, so 17 pass, 9 are excluded and 6 are `todo` with reasons (§6).

## 1. What `passing` means for a fuzz entry

A fuzz entry is `passing` when:

1. **The fuzzer is ported 1:1.** Its body and helpers are translated call by call, and every
   `Fuzz` read goes through the ported reader (`skia_rust_fuzz::Fuzz`, a port of `fuzz/Fuzz.h`
   and `Fuzz.cpp`), so a given input drives the Rust fuzzer through the same calls, with the same
   values, as Skia's. The reader's semantics are exact:
   - `next(T*)` copies `sizeof(T)` bytes in host order. Past the end it zero-fills the rest of
     the value and marks the input exhausted.
   - `next(bool*)` reads one byte and takes its low bit.
   - The `nextRange` **template** reads a raw `T` and clamps it to `[(T)min, (T)max]`. The casts
     truncate, so `FuzzNiceRegion(…, 300)` clamps a `uint8_t` to 44. With a float `T` it lets NaN
     through. The **float overload** `nextRange(float*, float, float)` maps the value into range
     with `fmod` instead. C++ overload resolution picks between them: `float` bounds select the
     overload, `int` bounds the template (`PolyUtils` reads its offset with `-1000, 1000`).
     `next_range` and `next_range_f32` keep the two apart, and each call is ported to the one
     C++ picks.
   - `nextEnum` clamps the enum's underlying integer to `[0, max]`, and `nextN` makes `n` calls
     to `next`.
2. **A deterministic replay runs it over its corpus without a panic or a hang.** The replay set
   is the empty input, the committed seeds in `fuzz/corpus/<Name>/` and the crash reproducers in
   `fuzz/regressions/<Name>/`. A panic covers a crash, a ported `SkASSERT_RELEASE`, an
   out-of-bounds index and an arithmetic overflow in debug builds. A hang is any input still
   running after 25 s, OSS-Fuzz's per-input timeout. A fuzzer with no seeds fails.
3. **`cargo xtask inventory verify` agrees.** Its fuzz leg runs `cargo test -p skia-rust-fuzz
   --lib` and maps each replay test `fuzz::<file_snake>::<Name>` back to
   `fuzz/<File>.cpp::<Name>`. `--update` flips entries to `passing` or `failing`, and
   `--only fuzz` runs this leg alone.

There is no oracle comparison. A fuzzer's output is "did not crash", and Skia's fuzzers have no
observable result beyond that. The value of the 1:1 port is that crash reproducers found for
Skia (or by our nightly run) mean the same thing on both sides.

**Two choices that depend on the ABI.** Skia's own behavior is platform-dependent in two places.
We follow the oracle's (Windows x64) ABI and note it at the call site:
- An unscoped enum with no fixed type (`SkPathOp`, `SkRegion::Op`) has `int` as its underlying
  type there, and `unsigned` on the Itanium ABI. Both read 4 bytes; only out-of-range values
  clamp differently.
- `char` is signed on x86 (`DrawFunctions`' `init_string`), so bytes from `0x80` up clamp to
  `0x20`.

**Not ported:** `SkDebugf`/`SkDEBUGF` traces and the `--verbose` dump of `Gradients`. They are
diagnostics with no effect on the run.

## 2. Seed corpora

Skia's corpora live in a cloud bucket (`gs://skia-cdn/oss-fuzz/`), not in its repository, so
they cannot be pinned or fetched in CI. **We generate seeds deterministically, commit them, and
test that they match the generator** (`skia_rust_fuzz::seeds`, regenerated with
`cargo run -p skia-rust-fuzz --bin fuzz-seeds`):

- **API fuzzers** (everything except the encoders) accept every byte string as input, so random
  bytes mostly hit the shallow paths. Each fuzzer gets **structured seeds** that pick each
  top-level branch with well-formed values (every `Pathop` choice and op, every gradient type
  with and without matrices, every `DrawFunctions` draw). They are written with `SeedWriter`,
  the inverse of `Fuzz`, and they line up exactly with the reads: the reader neither runs past
  the end nor leaves bytes unread, except where a branch is not taken. A few `SkRandom` byte
  strings of several lengths cover the rest.
- **Encoders** get seeds in `_MakeEncoderCorpus`'s format (width and height as `uint32_t`, then
  premultiplied N32 pixels), followed by the encoder's option bytes. They come from two small
  Skia resource images (`randPixels.png` and `mandrill_32.png`, decoded with our codec) and one
  synthetic image with alpha that needs no resources. Real images reach the encoders' filter
  and entropy paths in a way random pixels do not. The pixels are written as BGRA, the oracle's
  N32, so the seeds do not depend on the host.

The corpus is 157 files and about 110 KB. Seeds are little-endian; every platform we target is
little-endian. Without Skia's resources, the sync test checks every seed except the resource
images.

## 3. Crate layout

- **`fuzz/`: the `skia-rust-fuzz` crate, a workspace member on stable.** It contains the reader
  (`reader.rs`), `FuzzCommon` (`common.rs`, ported helper by helper as fuzzers need them), one
  module per Skia file (`src/fuzz/<file_snake>.rs`), the `FUZZERS` registry, the replay
  (`replay.rs`) and the seed generator (`seeds.rs`). `def_fuzz!(Name, fuzz_Name, |fuzz| { … })`
  defines the fuzzer and its replay `#[test]`. The crate builds on every CI platform, wasm32
  included, and its tests run in `cargo test --workspace`. Replay is deterministic, so it gates
  like any other test.
- **`fuzz/libfuzzer/`: the cargo-fuzz package (`skia-rust-fuzz-targets`).** It has its own
  `[workspace]` table, and the root `Cargo.toml` excludes it, so stable builds never see it or
  `libfuzzer-sys`. It has one `fuzz_targets/<Name>.rs` per registered fuzzer, each one line
  long: `skia_rust_fuzz::run("<Name>", data)`. Its `Cargo.lock` and run outputs are ignored.
  It type-checks on stable (`cargo check --manifest-path fuzz/libfuzzer/Cargo.toml`); running
  it needs nightly and `cargo-fuzz`.

## 4. Nightly job (proposal, not wired)

PLAN §6.6 lists "Fuzz (short run), Linux x64, nightly, non-gating". Proposed workflow
`.github/workflows/fuzz.yml`, triggered by `schedule` (daily) and `workflow_dispatch`, never by
pull requests:

```yaml
jobs:
  fuzz:
    runs-on: ubuntu-latest
    timeout-minutes: 60
    strategy: { fail-fast: false, matrix: { target: [CubicRoots, DrawFunctions, Gradients,
      JPEGEncoder, LegacyChromiumPathop, PNGEncoder, ParsePath, PathMeasure, Pathop,
      PolyUtils, QuadRoots, RegionOp, WEBPEncoder] } }
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
      - run: cargo install cargo-fuzz --locked
      - run: mkdir -p work && cargo fuzz run --fuzz-dir fuzz/libfuzzer ${{ matrix.target }}
             work fuzz/corpus/${{ matrix.target }} -- -max_total_time=240 -timeout=25 -rss_limit_mb=2048
      - if: failure()
        uses: actions/upload-artifact@v4
        with: { name: crash-${{ matrix.target }}, path: fuzz/libfuzzer/artifacts }
```

Four minutes per target is about 13 runner-minutes a night, on a public repository's free
runners. The job is not wired yet. A scheduled job nobody watches is noise, so the maintainer
turns it on together with an owner for its failures.

## 5. Crashes become regression tests

A crash found by the nightly job, by a local `cargo fuzz run`, or reported against Skia (OSS-Fuzz
bugs come with the input) is handled like this:

1. Minimize it (`cargo fuzz tmin`) and commit it as `fuzz/regressions/<Name>/<short-name>`. The
   replay picks it up automatically, so the fuzzer's test fails.
2. If Skia crashes on the same input too, it is a Skia bug: keep the input, mark the entry
   `failing` with a reason that links the upstream bug, and do not "fix" the port to diverge.
3. Otherwise it is a porting bug. Fix it like any other mismatch (CLAUDE.md, "When output
   doesn't match"), and the reproducer stays as the regression test.

## 6. Scope of the 32 entries

| Status | Entries |
|---|---|
| passing (17) | `CubicRoots`, `QuadRoots`, `ParsePath`, `PathMeasure`, `RegionOp`, `PolyUtils`, `Pathop`, `LegacyChromiumPathop`, `Gradients`, `DrawFunctions`, `PNGEncoder`, `JPEGEncoder`, `WEBPEncoder`, and from the FuzzCanvas slice `NullCanvas`, `RasterN32Canvas`, `ImageFilter`, `PDFCanvas` |
| excluded: Ganesh (6) | `MockGPUCanvas`, `NativeGLCanvas`, `CreateDDL`, `DDLThreadingGL`, `GrStyledShape`, `Triangulation` (`GrTriangulator` and `GrCpuVertexAllocator` are CPU code, but only Ganesh's path renderers use them) |
| excluded: not fuzzers (2) | `_DumpCanvas` (prints a `DebugCanvas` as JSON), `_MakeEncoderCorpus` (writes corpus files; §2 reuses its format) |
| excluded: compiled out (1) | `PolyUtils#2`, the empty body built with `SK_ENABLE_OPTIMIZE_SIZE` |
| todo: FuzzCanvas (3) | `RasterN32CanvasViaSerialization` (needs `SkPicturePriv::MakeFromBuffer`, the buffer reader of a serialized picture, not ported; Skia may signal a bug on every input there, needs an oracle check), `SerializedImageFilter` (needs `SkImageFilter::serialize` and `Deserialize`, not ported), `SVGCanvas` (M16: `SkSVGCanvas` is not ported) |

Some calls in the FuzzCanvas port are unported. Such a call keeps the reads that Skia makes for it, and only the drawing is left out, so the reader stays in step with Skia's. `SkPicture::makeShader` is ported (`PictureShaderExt::to_shader`, `make_fuzz_shader`'s picture case). `FuzzCreateValidInputsForRuntimeEffect` is not used by the FuzzCanvas fuzzers, so it is not ported. `SkMakeNullCanvas` is `SkNWayCanvas(0, 0)`, ported as a no-pixel canvas with the null canvas's hooks, since `SkNWayCanvas` itself is not ported.
| todo: other (3) | `Precompile` (Graphite precompile on a mock `Context`), `SkParagraph` (needs M10), `PNGRustEncoder` (needs codec wave C-R) |

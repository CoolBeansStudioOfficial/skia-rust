# Porting guide

How to port a piece of Skia to skia-rust. `CLAUDE.md` is the short rule list; this is the full version with the mechanics. Read both before your first task.

## 1. One task, start to finish

1. **Pick the task** (an assigned batch, or manifest entries with `status = "todo"`). A task is a set of Skia source files plus the unit tests that exercise them.
2. **Read the C++ first**: the header, the `.cpp`, and the tests. The Skia tree is `third_party/skia` at the pinned commit. Never port from memory or from other Rust ports (tiny-skia, skia-rs, etc.); port from this tree.
3. **Port the implementation** into the owning crate/module (§2), function by function, with a `// Port of:` link on each item (§4).
4. **Port the tests** 1:1 into `tests/src/…` (§6). `cargo xtask inventory module-path <id>` prints where each one goes.
5. **Run** `cargo xtask inventory verify --update`, which runs the ported tests and flips their manifest entries to `passing`.
6. **Check:** `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo xtask inventory verify`.
7. **Commit and open a PR** (§9). CI is the reviewer.

If a test still fails after 2 full attempts, follow §10 (escalation). Never weaken the test.

## 2. Where code goes

| Skia | skia-rust |
|---|---|
| `include/core/SkFoo.h`, `include/private/SkFoo.h`, `src/core/SkFoo.{h,cpp}` | `skia_rust_core::foo` (`crates/skia-rust-core/src/foo.rs`) |
| `src/core/SkFooPriv.h` (private helpers used by Skia's own tests) | `skia_rust_core::foo_priv`, a `#[doc(hidden)] pub mod` |
| `src/core/SkVx.h` (`skvx::`) | `skia_rust_simd::vx` |
| `src/opts/*`, per-CPU kernels | `skia_rust_simd` |
| `modules/skcms` | `skia-rust-skcms` crate |
| `src/effects`, `src/shaders` | `skia-rust-effects` (Phase 3) |
| `src/pathops` | `skia-rust-pathops` (Phase 4) |

- Module names are the snake-cased file name without `Sk` (`SkRRect` → `rrect`, `SkM44` → `m44`, `SkPathBuilder` → `path_builder`).
- One Rust module per Skia file pair. If a module grows past ~2,000 lines, split along the C++ file's own sections into `foo/` submodules.
- New crates follow the layering in PLAN §3.1. A crate never depends on one above it.

## 3. API: mirror skia-safe

The public API mirrors [rust-skia](https://github.com/rust-skia/rust-skia)'s **`skia-safe`** crate, the established idiomatic Rust API for Skia. `cargo xtask skia fetch` checks it out at `third_party/rust-skia` (pinned in `inventory/api-reference.toml`); read `skia-safe/src/<area>/<module>.rs` before naming anything. Users should be able to switch from `skia-safe` to `skia-rust` with as few edits as possible.

**Copy the API shape, never the code.** Port the implementation from Skia's C++; take only names, signatures, module paths and enum layouts from `skia-safe`.

What to match:
- **Names and paths:** type names (`Point`, `IPoint`, `Rect`, `IRect`, `RRect`, `Matrix`, `M44`, `Path`, `PathBuilder`, …), method names (`Point::normalize_vector`, `Matrix::rotate_deg_pivot`, `Rect::from_xywh`, …) and where items live (`skia_safe::matrix::TypeMask` → `skia_rust::matrix::TypeMask`; the facade re-exports types at the root as `skia-safe` does).
- **Signatures:** parameter order and kinds (`impl Into<Point>`, tuples like `(sx, sy)`, `&[Point]`), return types (`Option<Matrix>` from `invert`), `self` vs `&self`, builder-style `&mut Self` returns.
- **Types:** the `scalar` alias (`pub type scalar = f32`), public fields where `skia-safe` has them (`Point { x, y }`, `Rect { left, top, right, bottom }`), `bitflags` types where `skia-safe` uses them, and enums with the same variants.
- **Trait impls** `skia-safe` provides (`Default`, `PartialEq`, `Add`/`Sub`/…, `From` conversions), with Skia's semantics (see below).

Where `skia-safe` shows its FFI plumbing, use plain Rust instead and keep the user-facing shape:

| `skia-safe` | skia-rust |
|---|---|
| `Handle<T>` / `RefHandle<T>` value wrappers | a plain Rust struct with the same methods |
| `RCHandle<T>` (ref-counted, e.g. `Shader`, `Image`, `Data`) | a cheaply clonable type wrapping `Arc<…>`, with the same methods |
| `native()`, `native_mut()`, `from_native_c()`, `NativeTransmutable` | omitted |
| `skia_bindings::Sk…` enums re-exported as type aliases | a Rust `enum` with the same variant names and Skia's discriminants |

When `skia-safe` doesn't expose something (private `SkFooPriv` helpers, internals Skia's own tests use, newer m156 API), apply the mechanical rule:

| C++ | Rust |
|---|---|
| `SkFoo` | `Foo`, with `#[doc(alias = "SkFoo")]` |
| `fooBar()` | `foo_bar()`, with `#[doc(alias = "fooBar")]` when the name changed beyond case |
| nullptr / `bool` + out-param on failure | `Option<T>` |
| failure with a reason | `Result<T, E>` |
| `SkSpan<T>` / pointer + count | `&[T]` / `&mut [T]` |
| overloads | distinct names: a suffix describing the extra parameter (`map_points` / `map_points_to`) |
| `SkASSERT` / `SkUNREACHABLE` | `debug_assert!` / `unreachable!()` |
| `SkFooPriv::Bar` (test-only helpers) | free function `foo_priv::bar` in a `#[doc(hidden)] pub mod foo_priv` |

- **Equality must be Skia's:** if `operator==` compares floats with `==`, implement `PartialEq` the same way (`-0.0 == 0.0`, `NaN != NaN`). Don't add `Eq`/`Hash` where Skia's semantics would make them wrong.
- **Every public item** gets `#[doc(alias = "SkFoo")]` (and `#[doc(alias = "fooBar")]` for renamed methods) so Skia names are searchable.
- **Record deviations** in `docs/API_MAPPING.md`: any place skia-rust's public API differs from `skia-safe`'s, or where no `skia-safe` equivalent exists and the name isn't mechanical.
- Internal crates (`skia-rust-core`, …) use the module layout of §2; the `skia-rust` facade re-exports them along `skia-safe`'s paths.

## 4. Port comments and licence

Every ported item (fn, struct, impl block, const) carries a link to its source, with the line range at the pin:

```rust
// Port of: src/core/SkRect.cpp#L120-L134 (chrome/m156)
```

Each new file starts with:

```rust
// Copyright 2006 The Android Open Source Project (or the year/holder in the C++ file)
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: <list of C++ files>
```

## 5. Arithmetic: the exactness rules

Pixels must match Skia bit for bit, so arithmetic is translated, never paraphrased.

1. **Same types.** `float` → `f32`, `double` → `f64`, `int` → `i32`, `int64_t` → `i64`, `SkFixed` → `i32` (16.16), `SkFDot6` → `i32`. Do not widen to `f64` "for safety".
2. **Same order of operations.** `a + b + c` is `(a + b) + c`. Never reassociate, factor or simplify, and keep explicit temporaries where C++ has them.
3. **No implicit FMA.** The oracle is built with `-ffp-contract=off`. Use `f32::mul_add` only where Skia calls an explicit FMA (`std::fma`, `skvx::fma`, `SkScalarMulAdd` with FMA).
4. **Float → int conversions.** C++ `(int)x` truncates and is UB out of range; Rust `as` saturates and maps NaN to 0. Port Skia's own helpers (`sk_float_saturate2int`, `sk_float_round2int`, `SkScalarRoundToInt`, …) exactly and call those. Use a bare `as` only where Skia's code guarantees the value is in range, and say why in a comment.
5. **Integer overflow.** Where C++ wraps (unsigned) use `wrapping_*`. Where Skia checks overflow (`SkSafeMath`), port the check. Never rely on Rust's debug-mode overflow panics for behaviour.
6. **Rounding helpers and constants.** Port `SK_ScalarNearlyZero`, `SK_FloatInfinity`, `sk_float_round`, etc. with their exact definitions, including Skia's hex float constants.
7. **Libm.** `sqrtf`, `floorf`, `ceilf` and `fabsf` are exact in both languages. `sinf`, `cosf`, `tanf`, `atan2f`, `expf`, `logf` and `powf` are **not** guaranteed to match across libms. Port Skia's own approximations where Skia has them (`sk_float_*`). Otherwise use `f32` std functions and note it with `// skia-rust: libm`. The oracle tests will tell us if it matters.
8. **SIMD (`skvx`).** Lane-wise IEEE ops (`+ - * /`, `sqrt`, `min`/`max` with Skia's NaN semantics, comparisons) are exact in any implementation. Approximations (`approx_*`, `rcp`, `rsqrt` intrinsics) differ per CPU tier and must go through `skia_rust_simd` with the per-tier behaviour.

## 6. Porting tests

- One Rust module per Skia test file, at the path `cargo xtask inventory module-path` prints. Register it in the parent `mod.rs`.
- The file starts with the licence header and `// Port of: tests/FooTest.cpp (chrome/m156)`.
- `DEF_TEST(Name, reporter)` → `def_test!(Name, |reporter| { ... });`. Keep the name verbatim; verification maps it back to the manifest.
- Static helpers become private `fn`s that take `reporter: &mut Reporter`, with the same names snake-cased and the same order of calls.
- `REPORTER_ASSERT(r, cond)` → `reporter_assert!(r, cond)`; the message form also maps. `ERRORF` → `errorf!`, `INFOF` → `infof!`.
- **Never weaken an assertion.** No tolerances unless the C++ has the same tolerance (e.g. `SkScalarNearlyEqual`, which is ported as-is). No skipped loop iterations or dropped cases.
- **C++-only assertions** (pointer identity, `sizeof`, layout via `reinterpret_cast`): translate them to the closest Rust-observable property if one exists (e.g. `size_of::<Point>() == 8`). If none exists, keep the line as a comment starting `// skia-rust: not expressible in Rust:` with the reason. That is the only allowed omission.
- `SkRandom` is ported (`skia_rust_core::random`), so random-driven tests reproduce Skia's exact sequences.
- A test that exercises only Skia's own containers or utilities, where Rust uses `std` instead (`SkString`, `SkTArray`, `SkTDArray`, `SkTHash*`, `SkArenaAlloc`, `SkSpan`, `SkTSort`, `SkBitSet`, `SkStringView`, `SkSemaphore`, `SkOnce`), is **excluded**: set `status = "excluded"` and `reason = "tests Skia's <X>; Rust uses <Y>"` in the manifest, and don't port it.
- GMs (`gm/*.cpp`) have their own harness: see §11.

## 7. Clippy and lints

- `clippy::pedantic` is on, and CI treats warnings as errors.
- Ported arithmetic often trips `cast_possible_truncation`, `cast_sign_loss`, `cast_precision_loss`, `float_cmp`, `many_single_char_names` and `similar_names`. Allow these **on the item**, never module- or crate-wide, with a comment:
  ```rust
  #[allow(clippy::cast_possible_truncation)] // mirrors the (int) cast in SkRect::round
  ```
- No `unsafe` outside `crates/skia-rust-simd` (see `docs/UNSAFE.md`).

## 8. Dependencies

- Allowed now: `std` and `bitflags` (2.x, as `skia-safe` uses it). Adding any other crate needs a line in the PR description saying why it's needed and how it's licensed; `cargo deny` must pass.
- No dependency may replace ported Skia behaviour (e.g. no `half` crate for `SkHalf`: Skia's conversion rounding must be ported).

## 9. Git and PRs

- One branch per task: `port/<short-name>` (e.g. `port/rect`). Base it on the latest `origin/main`.
- Commit message: `port: <what>` + a body listing the manifest entries now passing, ending with the `Co-Authored-By` line from CLAUDE.md.
- PR title: `port: <what> (<N> tests passing)`. The body lists the manifest ids, any excluded entries with reasons, and any `skia-rust: not expressible` notes.
- Don't edit `inventory/manifest.toml` by hand except `status`/`reason` for exclusions; `verify --update` sets `passing`.
- Rebase on `origin/main` if CI or GitHub reports a conflict. Conflicts in `lib.rs`/`mod.rs` module lists are expected; keep both sides.

## 10. When a test won't pass

1. Find the first divergence: print intermediate values on both sides. You can build a tiny C++ harness against `third_party/skia/out/oracle/x64-sse2` if needed. Don't tweak code until the numbers happen to match.
2. After **2 full attempts**, stop. Write `notes/<file>-<test>.md` with what you tried, the expected vs actual values, the suspicious code and your best hypothesis, then mark the test `def_test!(#[ignore = "see notes/<file>-<test>.md"] Name, …)` and set its entry to `status = "failing"` with a one-line `reason` (CI runs `cargo test`, so a failing test must be ignored, never left red or deleted). Commit what passes, open the PR, and report the failure. The next model up the ladder (Sonnet → Opus) continues from your note.

## 11. Porting GMs

A GM is checked by rendering it exactly as DM does and comparing the SHA-256 of the bytes with the oracle's goldens (`oracle/README.md`), for every config (`8888`, `565`, `f16`) on every CPU tier. There are no tolerances: a GM passes when every hash matches. The harness is the `skia-rust-gm` crate in `tests/gm` (design: `docs/design/raster-pipeline.md` §4.4, "As implemented in A7").

**Where it goes.** `gm/<file>.cpp` → `tests/gm/src/gm/<file_snake>.rs`, registered in `tests/gm/src/gm/mod.rs` (sorted). `<file_snake>` is the file name snake-cased like test files (§6); a name starting with a digit gets a `_` (`gm/3d.cpp` → `gm::_3d`). The file starts with the licence header and `// Port of: gm/<file>.cpp (chrome/m156)`, and imports `use crate::prelude::*;`.

**Registrations.** Each `DEF_*GM*` becomes the macro with the same name and arguments, the body as a block:

| Skia | skia-rust |
|---|---|
| `DEF_SIMPLE_GM(name, canvas, W, H) { … }` | `def_simple_gm!(name, canvas, W, H, { … });` |
| `DEF_SIMPLE_GM_BG(name, canvas, W, H, BG) { … }` | `def_simple_gm_bg!(name, canvas, W, H, BG, { … });` |
| `DEF_SIMPLE_GM_BG_NAME(name, canvas, W, H, BG, NAME_STR) { … }` | `def_simple_gm_bg_name!(name, canvas, W, H, BG, "result_name", { … });` |
| `DEF_SIMPLE_GM_CAN_FAIL(name, canvas, errorMsg, W, H) { … }` | `def_simple_gm_can_fail!(name, canvas, error_msg, W, H, { …; DrawResult::Ok });` |
| `DEF_SIMPLE_GM_BG_CAN_FAIL` / `DEF_SIMPLE_GM_BG_NAME_CAN_FAIL` | `def_simple_gm_bg_can_fail!` / `def_simple_gm_bg_name_can_fail!` |
| `DEF_GM(return new FooGM;)` | `def_gm!(FooGM, FooGM::new());` |
| `DEF_GM(return new FooGM(true);)` | `def_gm!(FooGM_true = "FooGM(true)", FooGM::new(true));` |

The registration name must equal the manifest id's name verbatim (`gm/dashing.cpp::Dashing5GM(true)` → `"Dashing5GM(true)"`; a second registration with the same name in one file is `"FooGM#2"`): `cargo xtask inventory verify` maps the registry key `gm::<file_snake>::<name>` back to the manifest id. When the name is not an identifier, give the generated test an identifier and the name as a string (`FooGM_true = "FooGM(true)"`). `BG` is a `Color` (`SK_ColorWHITE` → `Color::WHITE`); `canvas` is a `&Canvas`; `error_msg` is a `&mut String`.

**Class GMs** (`class FooGM : public skiagm::GM`) implement the `GM` trait, one method per override:

| `skiagm::GM` | `GM` trait |
|---|---|
| `getName()` | `fn name(&self) -> String` |
| `getISize()` | `fn size(&mut self) -> ISize` |
| `GM(bgColor)` / `setBGColor()` | `fn bg_color(&self) -> Color` (default white; keep it in a field if `onOnceBeforeDraw` sets it) |
| `onOnceBeforeDraw()` | `fn on_once_before_draw(&mut self)` |
| `onDraw(SkCanvas*)` | `fn on_draw(&mut self, canvas: &Canvas)` |
| `onDraw(SkCanvas*, SkString*)` returning `DrawResult` | `fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult` |
| `modifySurfaceProps()` | `fn modify_surface_props(&self, props: &mut SurfaceProps)` |
| `onGpuSetup()` | `fn on_gpu_setup(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult` (DM calls it for raster too, without a GPU context) |

Constructor arguments become `FooGM::new(…)`; the GM's member variables become struct fields. Static helpers become private `fn`s with snake-cased names, as for tests.

**The canvas.** `Canvas` mirrors `skia-safe`'s (`&self` methods returning `&Self`). Until task D6 it is a stub (`tests/gm/src/canvas.rs`) that only clears; a GM whose drawing needs more panics with a "needs the real Canvas (task D6)" message, and its test fails. Port such GMs after D6.

**Running.** Each registration is also a `#[test]` named after it: `cargo test -p skia-rust-gm <name>` renders it under `force_tier` for every tier this host can check and every config, and fails on any mismatch, printing every comparison. On a mismatch the diff images are in `target/gm-diffs/<oracle tier>/<config>/<name>.png` (golden | ours | difference in magenta), with our bytes as `<name>.raw` and the golden as `<name>.golden.raw`. Then debug as in CLAUDE.md: `cargo xtask oracle rp-dump <tier> <gm>` for the oracle's raster-pipeline stages, never trial and error.

**Goldens.** The harness uses `goldens/<skia-commit>/` from the workspace, from the main checkout when run in a git worktree, or from `$SKIA_RUST_GOLDENS`; otherwise it downloads `hashes-m156.json` (5 MB) from the release in `inventory/goldens.lock`, checks its SHA-256 against the lock and caches it in `target/goldens/`. The objects archive (~540 MB) is downloaded the same way only when a mismatch needs a diff image. `SKIA_RUST_GOLDENS=release` forces the download path.

**Tiers.** Every GM is checked against every oracle tier that records its `Tier`'s behaviour (`Tier::oracle_tiers()`). `Scalar` runs natively; an x86 tier runs natively when the host has it and its `rcp`/`rsqrt` estimates match the oracle host's (AMD Zen 4), otherwise as its model with the oracle host's estimate tables (for `Ml4` the exact `vrcp14ps`/`vrsqrt14ps` model, so hosts without AVX-512 check `Ml4` too). `Neon` runs natively on arm64, otherwise as its model; it has no goldens yet. `cpu-x64-scalar` (the wasm proxy) is compared and reported but never decides a verdict.

**N32 byte order.** `8888` is `kN32_SkColorType`: BGRA on Windows, RGBA elsewhere. The default goldens come from a Windows host, so their `meta.json` says `BGRA_8888`. Bytes are never swizzled to make them match. On a host whose N32 is RGBA (macOS, Linux), `8888` is compared with the oracle's RGBA variant tiers (`-rgba`, built with `SK_R32_SHIFT=0`; `Tier::oracle_tiers_rgba()`, selected by `sink::uses_rgba_goldens`, a pure function of the config and the host order). Only the class representatives have a variant; a tier without a golden of the host's byte order is *not checkable* for `8888`, reported, never counted as passing and never as failing. `565` and `f16` do not depend on byte order, always use the default tiers and are checkable everywhere. Harness self-tests must not depend on the host: they compare bytes only on `565`/`f16`, and inject the host's N32 order (`Options::host_n32`) and tier selections instead of reading the real host.

**Done.** `cargo xtask inventory verify --update` runs every registered GM (`gm-verify`) and marks an entry `passing` only when all configs match on every oracle tier with goldens and every tier was checkable on this host; it marks mismatching entries `failing`. A GM that is not checkable on some host (CI runners without AVX-512 or with another vendor's estimates) keeps its status there; it is never a regression and never a pass. A GM that doesn't match after 2 attempts follows §10, with `def_gm!(#[ignore = "see notes/…"] …)`.

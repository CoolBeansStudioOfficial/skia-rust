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

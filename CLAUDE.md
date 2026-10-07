# skia-rust — agent instructions

A faithful port of Skia to safe, idiomatic Rust, measured by Skia's own tests matched **exactly** against real Skia. Read `docs/PLAN.md` for the full plan and `docs/PORTING.md` for the step-by-step porting mechanics (module layout, API mapping, arithmetic rules, test harness, PR flow); this file is the working rule set.

## Picking work
- Work comes from `inventory/manifest.toml`. Pick `status = "todo"` (or `"stale"`) entries; one entry or a small cohesive batch per PR.
- `cargo xtask inventory stats` shows progress. Never hand-edit generated fields (`id`, `kind`, `macro`, `source`, `span_hash`); only `module`, `status`, `reason`.
- The Skia source is at `third_party/skia` (pinned in `inventory/skia-pin.toml`; fetch with `cargo xtask skia fetch`). Read the C++ before writing Rust.

## Porting rules
1. **Port, don't invent.** Translate Skia's implementation function by function. Each ported item starts with a comment linking its source:
   `// Port of: src/core/SkScan_AntiPath.cpp#L120-L180 (chrome/m156)`
2. **Keep the arithmetic exact.** Same types (`f32`/`f64`/fixed-point/integer widths), same evaluation order, same rounding, same constants. Use `mul_add` only where Skia uses FMA. No reassociation, no "simplified" formulas, no swapping in `std` functions that round differently.
3. **Idiomatic Rust around the arithmetic:** ownership instead of refcounting where possible, `Arc` for shared immutable objects, `Option`/`Result` instead of null, enums/bitflags instead of int flags, iterators, no global mutable state.
4. **API shape:** mirror rust-skia's `skia-safe` public API (in `third_party/rust-skia`, see `docs/PORTING.md` §3): same names, paths and signatures, minus its FFI plumbing. Copy its API shape only, never its code. Where it has no equivalent: drop `Sk`, Rust casing. Always add `#[doc(alias = "SkCanvas")]` / `#[doc(alias = "drawRect")]`.
5. **No `unsafe`** outside `crates/skia-rust-simd`. Inside it: one operation per `unsafe` block, a `// SAFETY:` comment on each, and a scalar twin for every SIMD kernel that must produce bit-identical results.
6. **Lints:** `clippy::pedantic` must be clean (`cargo clippy --workspace --all-targets -- -D warnings`). An `#[allow(clippy::...)]` needs a comment saying why (typically: mirrors a C++ cast in ported arithmetic).

## Porting tests
- Tests are 1:1 translations: same name, same structure, same assertions, with a `// Port of:` link.
- Never weaken an assertion, widen a tolerance, or skip a case. There are no tolerances in this project.
- Never `#[ignore]` a test without changing its manifest status and writing a `reason`.

## Definition of done for a PR
- The manifest entries it targets are `passing`; nothing previously `passing` regressed.
- `cargo fmt --all --check`, clippy (above), and `cargo test --workspace` are clean.
- **CI is green on every platform** (Windows, Linux x64, macOS arm64, Linux aarch64, wasm32, Miri). Local runs on the Windows server don't catch: module `#[path]`/`include!` paths through `..` or non-existent directories (never use them), code that is dead or warns only on another `cfg` (CI builds with `RUSTFLAGS=-D warnings`), arch-specific intrinsics without `cfg(target_arch)` gates, and host-dependent test expectations (CPU tier, N32 byte order, path separators).
- For GMs: output hashes match the oracle goldens on every CPU tier the host supports.

## When output doesn't match
Use the oracle's debug dumps (raster pipeline stages, generated WGSL, path verbs) to find the first divergence before changing code. Don't adjust code by trial and error until the hash matches.

## Escalation (Sonnet → Opus)
After 2 full failed attempts on the same entry, stop and write `notes/<manifest-id>.md`: what you tried, the mismatching hashes/diffs, the relevant dumps, and your best hypothesis. An agent on the next more capable model continues from that file.

## Commands
```
cargo xtask skia fetch          # clone pinned Skia into third_party/skia
cargo xtask inventory sync      # rescan Skia, update manifest (preserves module/status/reason)
cargo xtask inventory stats     # pass rates by kind and module
cargo xtask inventory module-path <id>   # where a unit test's Rust port goes
cargo xtask inventory verify [--update]  # run ported tests; --update marks passing entries
cargo xtask oracle tiers        # oracle CPU tiers (server only for build/run; see oracle/README.md)
cargo xtask oracle compare <tier> <dir>   # check our outputs against a tier's golden hashes
cargo xtask oracle rp-diff [--update]     # per-stage raster pipeline oracle (docs/PORTING.md §12)
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

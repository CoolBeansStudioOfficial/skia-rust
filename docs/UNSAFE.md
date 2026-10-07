# Unsafe code policy

`unsafe` exists in skia-rust for one reason: SIMD. It is confined to `crates/skia-rust-simd`, and it must never be used to paper over sloppy code.

## Enforcement
- The workspace sets `unsafe_code = "deny"`. Only `skia-rust-simd` opts out (`#![allow(unsafe_code)]` in its `lib.rs`); CI fails if any other crate does.
- In `skia-rust-simd`, these are errors: `unsafe_op_in_unsafe_fn`, `clippy::undocumented_unsafe_blocks`, `clippy::multiple_unsafe_ops_per_block`, `clippy::missing_safety_doc`.
- Miri runs over the crate's scalar paths and safe wrappers on every PR.

## Rules
1. **One operation per `unsafe` block**, each with a `// SAFETY:` comment that states the invariant and why it holds here.
2. **Prefer safe `#[target_feature]` functions** (Rust ≥ 1.86). `unsafe` should be needed only to call a tier function after runtime CPU detection, and for raw unaligned loads/stores.
3. **Scalar twin for every kernel.** Tests assert SIMD and scalar outputs are bit-identical on random inputs (proptest) and on every golden.
4. **The public API is entirely safe.** No raw pointers cross the crate boundary.
5. **Review:** every new `unsafe` block is reviewed by an Opus agent (see `docs/PLAN.md` §8.3).

## Miri policy
The `miri` CI job (`cargo miri test -p skia-rust-simd`) gates every PR and must stay fast (target: under ~8 minutes; it had grown to ~24 as stage tests landed). Miri is ~1000x slower than native and cannot run the intrinsics, so it is used to check the *safe* wrappers, the scalar and model (`Model(AmdZen4)`/`Model(Arm)`) paths, register/array conversions, the kernels' slicing, and the estimate tables for UB. It is not a second runner for bit-exactness sweeps: the native test jobs run those on four platforms.

What runs under Miri:
- Everything that exercises `unsafe` or the safe wrappers around it, the Scalar tier, and the models.
- Known-answer tests, on Scalar and the `AmdZen4`/`Arm` models (the `Model(Host)` wrappers are covered by `Sse2` only).
- A small, representative sample of each stage family's twin/reference tests: a few stages, one full and one tail chunk, a few lengths around the vector widths.

What is reduced (never change what a test checks natively):
- Gate iteration counts, strides and sample lists with `cfg!(miri)` (see `rp/lanes/test_support.rs` `Budget`, `rp/tests_blend.rs` `rounds()`/`sample()`). Prefer a handful of values at the interesting boundaries (0, 1, tail, vector width, 65) over `step_by(small)`.
- Pure-arithmetic sweeps with no `unsafe` (exhaustive `u16`, strided `u32` bit patterns) use a coarse stride, or `#[cfg_attr(miri, ignore = "covered natively; too slow under Miri")]`.
- Tests of native tiers are compiled out or skipped (`!cfg!(miri) && tier.is_native()`).

When adding a stage family, write its tests so that the Miri run takes seconds: measure with `cargo +nightly miri test -p skia-rust-simd -- -Z unstable-options --report-time` and keep any single test under ~30 s there.

## Rejected uses
- `unsafe` to skip bounds checks without a benchmark proving it matters (restructure the loop instead).
- `transmute`, `static mut`, lifetime extension, `get_unchecked` in non-SIMD code.

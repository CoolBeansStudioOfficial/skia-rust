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

## Rejected uses
- `unsafe` to skip bounds checks without a benchmark proving it matters (restructure the loop instead).
- `transmute`, `static mut`, lifetime extension, `get_unchecked` in non-SIMD code.

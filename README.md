# skia-rust

A faithful port of the [Skia](https://skia.org) 2D graphics library to safe, idiomatic Rust, with a CPU raster backend and a Graphite-style GPU backend on [wgpu](https://wgpu.rs).

Progress is measured by one number: the share of **Skia's own tests** that pass, with image output matched **pixel-for-pixel** against real Skia.

> **Status:** Phase 0 (infrastructure). Nothing renders yet. See [docs/PLAN.md](docs/PLAN.md).

## Progress

Tracked against Skia `chrome/m156`. Regenerate with `cargo xtask inventory stats`.

| Kind | In scope | Passing |
|---|---:|---:|
| Unit tests | 2,572 | 0 |
| GMs (golden images) | 1,095 | 0 |
| Benchmarks | 1,280 | 0 |
| Fuzzers | 32 | 0 |
| SkSL compiler goldens | 924 | 0 |
| DM image decodes | 236 | 0 |
| DM Lottie renders | 190 | 0 |
| **Total** | **6,329** | **0 (0.0%)** |

Excluded items (Ganesh-only tests, SkSL backends not used by wgpu, disabled upstream) are listed with reasons in `inventory/manifest.toml`.

## License

BSD-3-Clause, the same as Skia. Code ported from Skia retains Google's copyright notice; see [LICENSE](LICENSE).

skia-rust is an independent project and is not affiliated with or endorsed by Google.

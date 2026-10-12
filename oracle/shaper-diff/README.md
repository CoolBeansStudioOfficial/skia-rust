# shaper-diff

HarfRust `=0.9.0` against HarfBuzz at Skia's pin (`9cb1fee5`, 13.1.0). Design:
`docs/design/modules.md` §2.4 and the results in §2.5.

| File | What |
|---|---|
| `cpp/shaper_diff.cc`, `build.sh` | The HarfBuzz driver. `build.sh <harfbuzz src dir> <binary>`; the sources come from `cargo xtask text-deps fetch`. |
| `src/` | The Rust twin (`shape.rs`), mode b's per-font tables (`tables.rs`), mode c's synthetic callbacks (`synthetic.rs`), the `shaper-diff` binary. |
| `configs.txt`, `fonts.txt`, `corpus/` | What both sides shape: modes and sizes, the 36 sfnt fonts of `inventory/test-fonts.lock`, the text sources (`gen_corpus.py` makes `corpus/` from the pinned Skia tree). |
| `expected/*.txt` | The driver's output per mode: `config font source lines glyphs sha256`. Committed. |
| `known-divergences.txt` | Rows where HarfRust differs, with a class (`classify.py` writes it). Every other row is exact. |
| `tests/replay.rs` | `cargo test -p skia-rust-shaper-diff`: shapes with HarfRust and compares with `expected/` and `known-divergences.txt`. No C/C++. |

```
cargo test -p skia-rust-shaper-diff               # replay (modes a and b fetch the test fonts)
cargo xtask oracle shaper-diff                    # rebuild HarfBuzz, check expected/ is current (Linux/macOS, g++)
cargo xtask oracle shaper-diff --update           # rewrite expected/ after a corpus or pin change, then:
python3 -I oracle/shaper-diff/classify.py target/shaper-diff/shaper_diff target/release/shaper-diff \
    <fonts dir> target/shaper-diff/tables           # rewrite known-divergences.txt
```

`SKIA_RUST_SHAPER_DIFF_FONTS=<dir>` replaces the fetched test fonts with a directory of your own (the
files must match `fonts.txt`). `SHAPER_DIFF_DUMP=1 shaper-diff run <a|b|c> --config NAME --font NAME`
prints every shaped line (`L config font source index glyphs`), as the driver's `--dump` does, for a
line-by-line diff of a divergent row.

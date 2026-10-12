# unicode-diff

`skia_rust_unicode::icu_shim` (M2: the ICU functions on ICU4X `=2.3.0` compiled data, with ICU's numbering
and locale handling; M4: break iterators that port ICU's `RuleBasedBreakIterator` and dictionary engines and
run Chromium's compiled break data from `skia-rust-unicode-data`) and `unicode-bidi =0.3.18` (M3), against
Chromium's ICU 78.2 at Skia's pin (`d578f2e8`). Design: `docs/design/modules.md` §3.3; results and the
divergence classes: §3.4 (M0c), §3.5 (M2), §3.6 (M4). It is the measuring tool for M2, M3 and M4; since M4
only bidi differs.

| File | What |
|---|---|
| `cpp/unicode_diff.cc`, `build.sh` | The ICU driver. It makes the calls of `SKICU_EMIT_FUNCS` (`SkUnicode_icupriv.h`) the way `SkUnicode_icu.cpp` and `SkUnicode_icu_bidi.cpp` do. `build.sh <icu common dir> <binary>`; the sources come from `cargo xtask text-deps fetch --only icu` (about 150 s on 4 cores for the ~240 `common/*.cpp`). |
| `src/` | The Rust twin: `breaks.rs` (`ubrk_*` through the shim), `bidi.rs` (`ubidi_*`), `reorder.rs` (a port of ICU's `ubidi_reorderVisual`), `props.rs`, `case.rs`, `misc.rs` (through the shim), `conformance.rs`; `lib.rs` compares and classifies. `$SKIA_RUST_UNICODE_DIFF_CORPUS` replaces `corpus/` (with the driver's `--corpus`, to compare cases of one's own). |
| `corpus/`, `gen_corpus.py`, `synthetic.txt` | The 869 strings both sides see: the 34 `resources/text` files (whole, as paragraphs), every string literal of the shaper, paragraph, Skottie and Unicode tests and the paragraph GM, the text layers of `resources/skottie`, and 178 hand-written cases (dots, colons, dictionaries, embeddings, special casing). |
| `expected/*.txt` | The driver's output per section. Committed. |
| `known-divergences.txt` | Every place the twin differs from ICU, with a class (`classify.py` writes it; the header explains the classes). Everything else is exact. |
| `tests/replay.rs` | `cargo test -p skia-rust-unicode-diff`: runs the twin and compares with `expected/` and `known-divergences.txt`. No C/C++. A divergence that is not listed fails, and so does a listed one that starts matching. |

Sections (one expected file each): `breaks` (line, word, character and sentence boundaries with the raw
`ubrk_getRuleStatus`, in UTF-8 and UTF-16, plus `ubrk_following` / `ubrk_preceding` at every offset),
`bidi` (`ubidi_getDirection`, `getLength`, `getLevelAt` for paragraph levels LTR, RTL, default LTR, default
RTL), `reorder` (`ubidi_reorderVisual` over 25,749 level sequences), `props` (`u_hasBinaryProperty` for the
six Skia properties, `u_getIntPropertyValue(UCHAR_LINE_BREAK)`, `u_iscntrl`, `u_isspace`, `u_isWhitespace`,
`uscript_getScript` over 0..=10FFFF), `case` (`u_strToUpper` for root, `tr`, `az`, `lt`, `el` over every
code point, and the corpus), `conformance` (the Unicode 17.0.0 `GraphemeBreakTest`, `WordBreakTest`,
`SentenceBreakTest`, `LineBreakTest` and `BidiCharacterTest` files, fetched by
`cargo xtask text-deps fetch --only ucd`), and `misc` (`u_errorName`, `uloc_forLanguageTag`,
`uloc_getDefault`). The `breaks` section opens `en` (the root rules), plus line and sentence cases for the
locales whose bundles select other rule files (`line-zh-u8`, `line-ja-u8`, `line-ja-normal-u8`,
`line-strict-u8`, `line-loose-u8`, `line-ko-strict-u8`, `sent-el-u8`).

```
cargo test -p skia-rust-unicode-diff               # replay (the conformance test fetches the UAX files)
cargo xtask oracle unicode-diff                    # rebuild ICU, check expected/ is current (Linux/macOS, g++)
cargo xtask oracle unicode-diff --update           # rewrite expected/ after a corpus or pin change, then:
python3 -I oracle/unicode-diff/classify.py target/unicode-diff/unicode_diff target/release/unicode-diff \
    <icudtl.dat> <ucd dir>                          # rewrite known-divergences.txt, print the class table
```

`SKIA_RUST_UCD_DIR=<dir>` replaces the fetched conformance files with a directory of your own (they are
verified against `inventory/text-deps.lock`). `unicode-diff run <section> [--dump]` prints a section in the
driver's format; `unicode-diff divergences <section>` lists every divergence of one.

# Design: Modules (Phase 7)

Status: proposed (2026-10-10). Pin: `chrome/m156` (`95ee33d7ec77`). Audience: every agent porting
Skia's `modules/skunicode`, `modules/skshaper`, `modules/skparagraph`, `modules/svg` (+ `src/svg`,
`src/xml`), `modules/skottie` (+ `modules/sksg`, `modules/skresources`, `modules/jsonreader`) and
`src/pdf`, their tests, GMs and benches. PLAN §8.3 assigns per-module design notes to Opus.

Skia references are `path#Lx-Ly` in the pinned tree (`third_party/skia`). Manifest counts are from
`inventory/manifest.toml` at `52e997b`. Golden facts are from `hashes-m156.json` of release
`goldens-m156` (`inventory/goldens.lock`). Library versions of HarfBuzz and ICU are those of Skia's
`DEPS` at the pin, read from their upstream mirrors at the pinned commits. The numbers in §2.3 and
§3.3 come from a pilot run on 2026-10-10 (Linux x64, clang 18.1 / rustc 1.99) of the pinned C
libraries against the candidate Rust crates. Appendix A says how to reproduce it.

## Decisions at a glance

1. **The oracle recorded nothing for these modules except 12 GM goldens.** `--src` is a parameter
   of `cargo xtask oracle run` (`xtask/src/oracle.rs#L549-L552`), and the published runs used
   `--src gm` only, with the configs `8888`, `565`, `f16` and `grdawn_*`: every key in
   `hashes-m156.json` is `<config>/gm/<name>`. There are no `lottie`, `svg`, `image`, `pdf` or
   `tests` results, and `oracle/` has no shaping, Unicode, PDF or SVG dumps. The module GMs with goldens are `paragraph_`,
   `paragraph__underline`, `paragraph__visitor`, `paragraph__underline_visitor`, five
   `skottie_*` GMs and three `pdf_*` GMs. Everything else is checked by its own unit-test
   assertions, or by a library-level differential harness (decisions 4 and 7), or it waits for an
   oracle (§1.3).
2. **The oracle shaped with HarfBuzz 13.1.0 and segmented with Chromium's patched ICU 78.2.** The
   pins are HarfBuzz `9cb1fee5` (`DEPS#L55`; `meson.build` says 13.1.0, one build-only commit after
   the tag) and `chromium/deps/icu@d578f2e8` (`DEPS#L57`; `uvernum.h` says 78.2) with its
   `common/icudtl.dat` (`third_party/icu/BUILD.gn#L21-L40`). HarfBuzz is built without ICU, so
   it uses its own Unicode tables, and with `HB_NO_FALLBACK_SHAPE` (`third_party/harfbuzz/BUILD.gn#L28-L36`).
   ICU4X and libgrapheme are off (`gn/skia.gni#L66-L69`). Chromium's ICU differs from stock ICU
   in its break rules and dictionaries (§3.1), and that difference is visible in the output.
3. **Shaping: depend on HarfRust `=0.9.0`; never port HarfBuzz wholesale.** 0.9.0 is the newest
   HarfRust that shares Skia's `read-fonts =0.40.1` (0.10 and later need `^0.40.2`), and the
   first line of releases with the `FontFuncs` and `scale` hooks that Skia's callbacks need (0.8.1+).
   It tracks HarfBuzz 14.2.0. The only shaping change HarfBuzz's NEWS lists between 13.1.0 and
   14.2.0 is "Indic: categorize U+1CF5 and U+1CF6 as CS". In the pilot (§2.3), HarfRust 0.9.0 and
   HarfBuzz at the pin produce identical glyph ids, clusters and advances for 2.56 million glyphs
   (39 test fonts × 34 scripts × 3 scales). 70 of 3,978 runs differ, all in one class: the
   fallback mark offset of `.notdef` marks in Arabic/Syriac text set in fonts without those
   scripts.
4. **Exactness is proven against the pinned C libraries, not against Skia.** New tools
   `oracle/shaper-diff` and `oracle/unicode-diff` (tasks M0b, M0c) build HarfBuzz and Chromium ICU
   at Skia's pins on Linux, the way `oracle/codec-diff` builds zlib and libpng. They need no
   Windows oracle host. They store expected results in the repo, and `cargo test` replays them on
   every platform. HarfBuzz builds in about 75 s and ICU's `common` library in about 150 s on 4 cores.
5. **Divergence policy for shaping.** For each class shaper-diff finds, there are three options,
   in this order: (a) it disappears under Skia's own font callbacks, which replace HarfRust's builtin
   extents and advances, so there is nothing to do; (b) fix it upstream in HarfRust and carry the
   fix as a vendored patch crate (`third_party/harfrust`, a path dependency, never git) until a
   compatible release has it; (c) only if (b) is refused, port the HarfBuzz function concerned
   into that patch. A full HarfBuzz port (about 40k lines of shaping C++) is rejected.
6. **Unicode: port Skia's ICU code path, not its ICU4X path, over an ICU-API shim backed by
   ICU4X.** The oracle ran `SkUnicode_icu.cpp` and `SkUnicode_icu_bidi.cpp`. They call 30 ICU
   functions (`SkUnicode_icupriv.h#L21-L52`). We port both files function by function and implement
   those 30 functions in a small shim on ICU4X (`icu_segmenter`, `icu_properties`, `icu_casemap`)
   and on `unicode-bidi` with ICU4X data. `SkUnicode_icu4x.cpp` is not ported: it behaves
   differently and its pin (ICU4X 1.4.0, `DEPS#L58`) is two years old.
7. **Bidi via `unicode-bidi` is exact; segmentation needs Chromium's data.** In the pilot, ICU and
   `unicode-bidi 0.3.18` with ICU4X 2.3.0 data resolve the same embedding level for every code
   point of 2,727 runs, and pass all 91,707 cases of `BidiCharacterTest-17.0.0`. Grapheme and
   sentence boundaries agree on all 909 corpus strings. Line and word boundaries do not, for three
   reasons, all of them Chromium patches or ICU dictionary behaviour (§3.3). So the shim uses ICU4X
   engines with **custom ICU4X data generated from Chromium's ICU tree** (task M4). Until M4 lands,
   unicode-diff lists the remaining classes, and no entry that depends on them flips.
8. **Paragraph tests are self-checking, but they need real fonts and a real font backend.** 153 of
   157 `SkParagraphTest` cases are `UNIX_ONLY_TEST`, and nearly all of them return early without
   `--paragraph_fonts` (`SkParagraphTest.cpp#L283-L296`). Skia's own Linux
   `NativeFonts_Fontations` bot runs them with `--nativeFonts --paragraph_fonts extra_fonts
   --norun_paragraph_tests_needing_system_fonts --fontations`
   (`infra/bots/gen_tasks_logic/dm_flags.go#L1820-L1834`), that is, Fontations, HarfBuzz and ICU. We
   reproduce that configuration: the `skparagraph` asset fonts (open question Q2), the text
   design's `NativeFontations` configuration (`docs/design/text.md` §8), and our shaper and
   Unicode code. Their C++ tolerances (`EPSILON*`) are ported as they are.
9. **Skottie `dm-lottie` entries (187) stay `todo` with `needs-oracle`.** DM renders each Lottie
   file as a 1000×1000 filmstrip of 5×5 frames (§5.1), but no lottie golden was ever produced. The
   three `.png` entries are not lottie sources (DM gathers `*.json` only, `dm/DM.cpp#L929`) and are
   excluded. Skottie still gets real verification: 5 GM goldens, 22 unit tests, `SGTest` (2). Skia
   ships no expression engine (`modules/skottie/include/Skottie.h#L80`, an interface only), so
   none is ported.
10. **SVG: pull the DOM core forward and do it first.** It unblocks `TestSVGTypeface` (text T20,
    13 entries, text.md Q1). The glyph SVGs in `resources/fonts/svg` use only `svg`, `g`, `defs`,
    `path`, `rect`, `circle`, `ellipse`, `transform`, `viewBox`, inline `style` and stroke and
    opacity properties. The SVG canvas (`src/svg/SkSVGDevice.cpp`) is tested by comparing XML text,
    so its `printf("%g")` and `"%.8g"` output (`#L164-L176`, `#L1283-L1284`) must be reproduced exactly.
11. **PDF: no current test compares a whole PDF byte for byte, but the port is built so that a
    later one could pass.** The tests compare serialized primitives exactly (`PDFPrimitivesTest`),
    look for substrings in the document (`PDFDocumentTest`), check that the source JPEG appears
    verbatim inside the PDF (`PDFJpegEmbedTest`), and round-trip deflate. Deflate is the existing
    port of Chromium zlib (`crates/skia-rust-zlib/src/deflate.rs`). That is already the byte-exact
    path (codecs.md decision 4). Font subsetting is the one place where the default build uses
    a library we cannot match exactly (HarfBuzz `hb-subset`, `BUILD.gn#L1262-L1279`); see Q4.
12. **Work order (§9):** bookkeeping and the two differential tools (M0), then the parts with no
    shaping dependency, run in parallel: Unicode shim, shaper skeleton, XML and SVG core, JSON
    reader, PDF primitives. Then HarfBuzz-on-HarfRust (M6), which unlocks paragraph, Skottie
    text, SVG text and the module GMs. In scope after M0a: 286 unit entries (paragraph 156,
    shaper 36, PDF 33, SVG 23, Skottie 22, Unicode 14, sksg 2), 10 module GMs, 3 `core` unit tests
    that need the PDF backend, the 13 text entries blocked on T20, and 98 benches (Skottie 50,
    shaper 34, PDF 12, paragraph 2). 187 `dm/lottie` entries wait for an oracle.

---

## 1. What the oracle built, and what it recorded

### 1.1 Build facts

| Fact | Source | Consequence |
|---|---|---|
| `skia_use_harfbuzz = true`, `skia_use_icu = true`, `skia_use_icu4x = false`, `skia_use_libgrapheme = false`, `skia_use_client_icu = false`, `skia_use_bidi = false` | `gn/skia.gni#L48-L69`; `oracle/tiers.toml` `[gn] args` sets none of them | `SkShapers::BestAvailable()` = HarfBuzz + `SkUnicodes::ICU` (`modules/skshaper/utils/FactoryHelpers.h`) |
| HarfBuzz `9cb1fee5` = 13.1.0 + "[meson] Update wraps" | `DEPS#L55`; upstream `meson.build`, `NEWS` | shaping semantics of 13.1.0 |
| HarfBuzz defines: `HAVE_OT`, `HB_NO_FALLBACK_SHAPE`, `HB_NO_WIN1256`, config override; **no** `HAVE_ICU` | `third_party/harfbuzz/BUILD.gn#L28-L36` | HarfBuzz uses its built-in UCD (Unicode 17), as HarfRust does |
| ICU = Chromium `deps/icu@d578f2e8` (78.2) with `common/icudtl.dat` | `DEPS#L57`; `third_party/icu/BUILD.gn#L21-L40` | Chromium's break-rule and dictionary patches apply (§3.1) |
| `skia_enable_skottie`, `skia_enable_svg`, `skia_enable_pdf` true; `SK_ENABLE_SKOTTIE_SKSLEFFECT` | `gn/skia.gni#L30-L33`; `modules/skottie/BUILD.gn#L11-L12` | all module GMs compiled into DM |
| `skia_pdf_subset_harfbuzz = skia_use_harfbuzz` → `SK_PDF_USE_HARFBUZZ_SUBSET` | `gn/skia.gni#L163`; `BUILD.gn#L1262-L1279` | PDF subsets fonts with `hb-subset` |
| `skia_use_expat = true` | `gn/skia.gni#L55` | `SkXMLParser`/`SkDOM` (SVG DOM, XMP) on expat |
| DM ran `--nativeFonts false`, and the published runs used `--src gm` | `xtask/src/oracle.rs#L519-L552`; the keys of `hashes-m156.json` | module GMs used the portable `TestTypeface` (text.md §1) |
| `UNIX_ONLY_TEST` is `DEF_TEST_DISABLED` on Windows | `tests/Test.h#L306-L310` | irrelevant to goldens, since tests were not run; it matters only for which tests Skia's own bots run (§4) |

### 1.2 The module goldens

All present on every CPU tier (`8888`/`565`/`f16`), on the RGBA variant tiers (`8888`), and on the
three GPU tiers. `paragraph_layout_` returns `kSkip` in GM mode (`modules/skparagraph/gm/simple_gm.cpp#L238-L240`)
and has no golden. That is a skip-match (text.md §1.2).

| GM (manifest id) | Golden name | Size | What it exercises |
|---|---|---|---|
| `modules/skparagraph/gm/simple_gm.cpp::ParagraphGM(0)` | `paragraph_` | 412×420 | `ParagraphBuilderImpl` + `layout(400)` of an English speech text in `sans-serif` 30 pt via `TestFontMgr()` (`simple_gm.cpp#L75-L117`): ICU line breaks, HarfBuzz on an **empty face** (§2.1), painting |
| `…ParagraphGM(kUseUnderline)` / `(kShowVisitor)` / `(kShowVisitor \| kUseUnderline)` | `paragraph__underline`, `paragraph__visitor`, `paragraph__underline_visitor` | 412/810×420 | decorations (through-mode underline), `Paragraph::visit`, `Font::getIntercepts` |
| `…ParagraphGM(kTimeLayout)` | none (skip) | — | skip-match |
| `modules/skottie/gm/SkottieGM.cpp::SkottieWebFontGM` | `skottie_webfont` | 800×800 | text layer with a fake web-font provider, portable fonts |
| `…SkottieColorizeGM("color", …)`, `("text", …)` | `skottie_colorize_color`, `skottie_colorize_text` | 800×800 | property observers; text animator (`skottie-text-animator-5.json`) |
| `…SkottieMultiFrameGM` | `skottie_multiframe` | 800×800 | `skresources::MultiFrameImageAsset` over a codec |
| `modules/skottie/gm/ExternalProperties.cpp::SkottieExternalPropsGM` | `skottie_external_props` | 800×800 | slot/property manager, text |
| `gm/pdf_never_embed.cpp::{pdf_never_embed, pdf_crbug_772685, pdf_table_based_subset}` | same | — | raster GMs (no PDF backend). Already scheduled in text wave 2 (text.md §9) |

### 1.3 What can be verified without a new Skia oracle run

| Evidence | Covers | Status |
|---|---|---|
| The 12 module GM goldens above | paragraph and shaper on the portable empty-face path, Skottie core/text/images | available now |
| Unit-test assertions | 286 module unit entries (§9) | available now. Paragraph needs the `skparagraph` font asset (Q2) |
| `oracle/shaper-diff` (M0b): HarfBuzz at the pin vs HarfRust | glyph ids, clusters, advances, offsets, flags on any corpus, with Skia's callbacks | needs M0b; buildable on any Linux host |
| `oracle/unicode-diff` (M0c): Chromium ICU 78.2 vs our shim | every ICU call Skia makes | needs M0c; buildable on any Linux host |
| `oracle/codec-diff/zlib` (exists) | PDF deflate bytes | done (codecs C-wave) |
| Unicode conformance files (`BidiCharacterTest`, `LineBreakTest`, …) | UBA/UAX#14/#29 baselines | public data |
| **Not verifiable:** `dm/lottie/*` (187), SVG files as DM sources (no manifest entries), PDF and SVG sink output, any module output rendered through Fontations (text.md §7.3), GPU module GMs (gpu.md §1) | — | `needs-oracle`; see Q6 |

---

## 2. Shaping: HarfRust vs HarfBuzz

### 2.1 How Skia drives HarfBuzz

`SkShaper_harfbuzz.cpp` does not let HarfBuzz read metrics from the font. It installs a sub-font
whose callbacks call Skia's `SkFont` (`#L211-L238`):

- `nominal_glyph(s)` → `SkFont::unicharToGlyph` / `textToGlyphs` (`#L89-L127`). Variation selectors
  fall through to the parent `hb_ot` font.
- `glyph_h_advance(s)` → `SkFont::getWidth(s)`, rounded to an integer when the font is not subpixel,
  then `skhb_position(v) = SkScalarRoundToInt(v * 65536)` (`#L72-L76`, `#L129-L180`).
- `glyph_extents` → `SkFont::getBounds`, `roundOut` when not subpixel, with the y axis flipped (`#L182-L208`).
- The scale is `skhb_position(font.getSize())` on both axes (`#L339-L356`). Variations come from
  `getVariationDesignPosition` (`#L313-L337`). The cluster level is `MONOTONE_CHARACTERS` (`#L1443`).
  Script, language and direction come from Skia's run iterators (`#L1466-L1475`).
- The face (`#L274-L311`) is built from `openExistingStream` when that is an sfnt. Otherwise it is
  built table by table through `copyTableData`, with `hb_face_set_upem(getUnitsPerEm())`.

**The empty face.** The portable `TestTypeface` has no tables (`tools/fonts/TestTypeface.h#L124`), a
non-sfnt stream (`TestTypeface.cpp#L166`) and an upem of 2048 (`TestTypeface.h#L107`). So in every
module golden, HarfBuzz shaped with an empty `hb_face_create_for_tables` face. That means no
GSUB/GPOS/GDEF, glyph count 0, and every metric from Skia's callbacks. Shaping there reduces to
normalization, default-ignorable handling, mirroring, fallback mark positioning (from the
extents callback) and fallback spaces. HarfRust needs a `FontRef`, so M6 synthesizes a minimal
in-memory sfnt (`head` with the typeface's upem, nothing else) and supplies everything else
through `FontFuncs`. shaper-diff mode (c) checks that this matches HarfBuzz's empty face.

### 2.2 Version choice

| HarfRust | HarfBuzz tracked (CHANGELOG) | `read-fonts` | `FontFuncs`/`scale` | Usable? |
|---|---|---|---|---|
| 0.5.1–0.5.2 | 13.0.0 | `^0.37` | no | no: cannot express Skia's callbacks |
| 0.6.x–0.8.0 | 14.1.0–14.2.0 | `^0.39` | no (`FontFuncs` from 0.8.1) | no |
| 0.8.1–0.8.4 | 14.2.0 | `^0.39` | yes | possible, but duplicates `read-fonts` beside Skia's 0.40.1 |
| **0.9.0** | **14.2.0** | **`^0.40.0`** | **yes** | **yes**: one `read-fonts` (=0.40.1) for skrifa and HarfRust |
| 0.10.0–0.11.0 | 14.x | `^0.40.2` | yes | no: conflicts with `read-fonts =0.40.1` (text.md §2.2) |
| 0.12+ | 14.x | `^0.41`… | yes | no: second `read-fonts` |

No HarfRust release tracks 13.1.0. HarfBuzz between the pin and 14.2.0: NEWS for 14.0.0, 14.1.0
and 14.2.0 lists one shaping change (U+1CF5/U+1CF6 Indic category, 14.2.0). Between 13.0.0 and
the pin, the source log has two shaping commits, both for the Arabic `stch` feature
(`7b7d54e33b`, `708bf4a0c8`), and HarfRust 0.9.0 has both. HarfRust's own `HARFBUZZ.md` lists its
known differences from HarfBuzz's test suite: `SHBALI-3` (rounding at unusual upem), Arabic fallback
positioning, DFONT collections and vertical extents fallback. Its README also says that malformed fonts
return an error where HarfBuzz falls back. A scratch crate with `harfrust =0.9.0`,
`read-fonts =0.40.1` and `skrifa =0.43.2` resolves to a single `read-fonts 0.40.1` (pilot).
Licences: MIT; dependencies `bytemuck`, `smallvec`, `bitflags` (MIT/Apache/Zlib), all on the
`deny.toml` allowlist.

### 2.3 Pilot measurement (HarfBuzz at the pin vs HarfRust 0.9.0)

Both sides use their built-in OpenType font functions (not Skia's callbacks), cluster level
monotone characters, `guess_segment_properties`, one run per line. Corpus: the 34
`resources/text/*.txt` scripts × every font in `resources/fonts` (39) × scale {none, 12 pt, 30 pt in
16.16}.

| Measure | Result |
|---|---|
| Runs / lines / glyph records | 3,978 / 12,051 / 2,556,258 |
| Runs byte-identical (glyph, cluster, x/y advance, x/y offset) | 3,908 (98.2 %) |
| Differing runs | 70 (245 lines): `arabic.txt` (2 lines) and `syriac.txt` (5 lines) in 13 fonts that cover neither script |
| Nature of every difference | `y_offset` of a combining mark mapped to glyph 0. HarfBuzz gives an extents-based offset (e.g. ±1062 in `7630.otf` font units), HarfRust gives 0. Glyph ids, clusters and advances are equal everywhere |

This does not yet cover Skia's callbacks, the empty face or the paragraph-test fonts. Those are
M0b's modes (b) and (c), and its corpus adds `extra_fonts`.

### 2.4 What M0b must establish before M6 flips anything

1. Mode (a), builtin funcs: the class above, and the root cause of the `.notdef` mark offsets.
2. Mode (b), Skia's callbacks: the Rust side writes, per (font, size, glyph), the 16.16 advances,
   the extents and the nominal mapping that skia-rust's `Font` produces. The C side installs them as
   `hb_font_funcs` exactly as `skhb_get_font_funcs` does, so both sides shape with identical inputs.
3. Mode (c), the empty face: `hb_face_create_for_tables` with no tables and upem 2048 vs the
   synthesized sfnt.
4. Corpus: `resources/text/*`, every string literal of the paragraph, shaper and Skottie tests,
   the paragraph GM's speech, the text layers of `resources/skottie/*.json`, and the fonts of
   `resources/fonts` and `extra_fonts`.

The expected output is committed as `oracle/shaper-diff/expected/*.txt` with a hash of each case,
the way rp-diff does it (PORTING §12).

---

## 3. Unicode: ICU4X vs Chromium ICU 78.2

### 3.1 What Skia calls, and what the oracle's ICU is

`SkUnicode_icu.cpp` (708 lines) and `SkUnicode_icu_bidi.cpp` (153) reach ICU only through the
`SKICU_EMIT_FUNCS` table (`SkUnicode_icupriv.h#L21-L52`): `ubrk_*` (open, setUText, first, next,
following, preceding, current, getRuleStatus, clone), `utext_openUTF8/openUChars`,
`ubidi_*` (openSized, setPara, getLevelAt, getDirection, getLength, reorderVisual),
`u_hasBinaryProperty` (emoji, emoji component/modifier/modifier base, regional indicator,
ideographic), `u_getIntPropertyValue(UCHAR_LINE_BREAK)`, `u_isspace`, `u_isWhitespace`,
`u_iscntrl`, `u_strToUpper`, `uscript_getScript`, `uloc_*`. Hard line breaks are
`ubrk_getRuleStatus(...) == UBRK_LINE_HARD` (`SkUnicode_icu.cpp#L395-L415`, `#L575-L583`).

Chromium's ICU carries break-iterator patches (`chromium/deps/icu@d578f2e8:README.chromium`, "D.2"):
`wordbrk.patch` moves `.` and `．` from MidNumLet to MidNum and removes `:` from MidLetter;
`line_normal.patch` makes CSS `line-break: normal` the default; `lstm.patch` replaces LSTM with
dictionaries for Thai and Burmese; `khmer-dictbe.patch` and an older `laodict.txt` use smaller
dictionaries; `cjdict.patch` adds Chinese words. Stock ICU 78.2 would therefore not reproduce the
oracle either.

### 3.2 Decision

- Port `SkUnicode.cpp`, `SkUnicode_hardcoded.cpp`, `SkUnicode_icu.cpp`, `SkUnicode_icu_bidi.cpp`,
  `SkBidiFactory_icu_full.cpp` and `SkUnicode_client.cpp` function by function into
  `skia-rust-unicode`.
- `unicode::icu_shim` implements the 30 functions with ICU's contracts, including the quirks the
  callers observe:
  - Dictionary boundaries report the rule status of the rule-based boundary that ends their
    range. Measured: in `lao.txt`, the 25 dictionary boundaries between bytes 191 and 427 report
    status 100 (`UBRK_LINE_HARD`), because the range ends at the paragraph break at byte 449, so
    Skia marks them as **hard** line breaks.
  - `u_isWhitespace` and `u_isspace` have ICU's (Java/POSIX) definitions, not Rust's.
  - `u_strToUpper` follows ICU's locale rules.

  Each function gets unicode-diff cases.
- Engines: `icu_segmenter`, `icu_properties`, `icu_casemap` (ICU4X 2.x, `=`-pinned, Unicode-3.0
  licence), and `unicode-bidi =0.3.18` with ICU4X's `BidiClass` data (`icu_properties` feature
  `unicode_bidi`). `ubidi_reorderVisual` is a 40-line port from ICU `ubidiln.cpp`.
- Data: ICU4X's compiled data is the starting point. M4 generates baked ICU4X data from Chromium's
  ICU tree (its `icuexport` output plus the patched rule definitions and dictionaries) into a
  `skia-rust-unicode-data` crate, so the segmenters reproduce §3.1.
- The SkUnicode test variants map like this: `Compiled_*` (ICU) and `ICU4X_*` run against this one
  implementation. `SkUnicode_Client` runs the client port. `SkUnicode_GetUtf8Words` is libgrapheme-only
  and is excluded (backend not built, `gn/skia.gni#L69`).

### 3.3 Pilot measurement (Chromium ICU 78.2 vs ICU4X 2.3.0 / unicode-bidi 0.3.18)

ICU's `source/common` and `common/icudtl.dat` at the pin were built and loaded with
`udata_setCommonData`, and called the way `SkUnicode_icu.cpp` calls them (`utext_openUTF8` +
`ubrk_open(…, "en")`). The corpus is 909 strings: the 34 `resources/text` files plus 875 string
literals from `SkParagraphTest.cpp`, `simple_gm.cpp` and Skottie's `Text.cpp`/`Shaper.cpp`.

| Service | Strings differing | Cause |
|---|---|---|
| Grapheme clusters | 0 / 909 | — |
| Sentences | 0 / 909 | — |
| Bidi embedding levels (auto, LTR, RTL paragraph level) | 0 / 2,727 runs (130,623 code points) | — |
| `BidiCharacterTest-17.0.0` (unicode-bidi + ICU4X data) | 0 / 91,707 | — |
| Line breaks, ICU4X default (`Strict`) | 7: `kana`, `thai`, `lao`, `khmer`, `myanmar`, `newtailue`, `taitham` | `kana`: `line_normal`. Others: SA-script dictionaries (ICU4X compiled data uses LSTM or other dictionaries, and has none for New Tai Lue / Tai Tham where ICU does not break) |
| Line breaks, `Normal` strictness | 6 (all SA scripts) | as above. No literal from any test differs |
| Word boundaries | 211 | 404 boundaries around `.` in 200 strings (`wordbrk.patch`; e.g. ICU breaks `SkCanvas.h` at the dot), 187 in 11 CJ/SA files (dictionaries) |

So unicode-bidi and ICU4X data are exact now. Segmentation is exact for every non-SA script once
Normal strictness is chosen, except words around `.`/`:`, which M4's patched rules fix. SA
dictionaries also need M4. The hard-status quirk needs the shim.

---

## 4. Paragraph (`modules/skparagraph`, 8.5k C++ lines)

- **Tests.** `SkParagraphTest.cpp`: 157 registrations. 4 are `DEF_TEST_DISABLED` (excluded) and 153
  are `UNIX_ONLY_TEST`. Fonts come from `ResourceFontCollection`, which loads `--paragraph_fonts`
  through `ToolUtils::TestFontMgr()->makeFromStream` (`#L118-L203`). The fonts are the
  `skparagraph` CIPD asset v4 (`infra/bots/assets/skparagraph/create.py#L40-L60`):
  `Rusino/textlayout@9c1868e8` `fonts/` plus Skia's `resources/fonts` at `2f82ef6e` plus Noto
  Naskh Arabic (sha256-checked). Families used: Roboto (103×), Ahem (34), Noto Naskh Arabic,
  Noto Color Emoji, Source Han Serif CN, Noto Sans CJK JP, Homemade Apple, Katibeh, Google Sans,
  Droid Serif. Three tests (`NonMonotonicGlyphsLTR/RTL`, `MultiStyle_Zalgo`) need system fonts and
  return early under the bots' `--norun_paragraph_tests_needing_system_fonts`. We mirror the bot,
  and their PR states that the entries are hollow.
- **Configuration.** `ResourceFontCollection` needs a manager that loads font files, so these tests
  run only under the text note's `NativeFontations` configuration (T19b). That is the
  configuration of Skia's `NativeFonts_Fontations` bots, where the tests pass with Fontations +
  HarfBuzz + ICU. An entry is `passing` when it passes there with the fonts present. Under
  `Portable` it returns early, as in Skia, and is never counted.
- **GMs** (§1.2) run on the portable configuration and pin down the empty-face shaping path,
  English line breaking, decorations and the visitor API exactly.
- `SkShaperJSONWriterTest` (3) tests a test-only helper (`tests/SkShaperJSONWriter.cpp`) and ports
  with it. `ParagraphBench` (2), `FuzzSkParagraph` (1).

---

## 5. Skottie (`modules/skottie` 19.5k, `sksg` 4.7k, `skresources` 0.8k, `jsonreader` 1.4k)

### 5.1 What `dm-lottie` would compare, and why it can't yet

`SkottieSrc::draw` (`dm/DMSrcSink.cpp#L1299-L1357`):

- Resources come from `FileResourceProvider(dirname, kPreDecode)` behind
  `DataURIResourceProviderProxy`, with `ToolUtils::TestFontMgr()` (portable in the oracle).
  Precomps go through `ExternalAnimationPrecompInterceptor("__")`, and shaping through
  `SkShapers::BestAvailable()`.
- `kPreferEmbeddedFonts` is set only with `--useLottieGlyphPaths`.
- The canvas is cleared to white, then 25 frames are drawn at `t = k/24`, each into a
  200×200 tile of a 1000×1000 surface (`DMSrcSink.h#L324-L329`). Rows and columns are visited in
  the order {4, 0, 3, 1, 2}, to exercise non-monotonic seeking. Each tile is drawn under an AA
  `clipRect` with a center-fit `RectToRect` matrix.
- Raster, GPU and vector sinks are accepted, but only with the direct approach (`#L1365-L1372`).

Skia's bots run lottie only on `Lottie` jobs, with the `lottie-samples` asset (VERSION 3), not
`resources/skottie` (`dm_flags.go#L1102-L1111`, `#L1865-L1868`). Our manifest's 187 JSON entries
come from `resources/skottie` (`xtask/src/inventory.rs#L340-L347`). No tier has a lottie result.
They stay `todo` with the reason `needs-oracle: no DM --src lottie goldens (oracle ran --src gm
only)`, the same wording as the `dm-image` entries. M23 ports `SkottieSrc` into the GM harness so
that the renders exist and are deterministic across our platforms.

### 5.2 Dependency chain

```
jsonreader (SkJSONReader: fast path + std::from_chars fallback, SkJSONReader.cpp#L305-L324, #L873-L889)
skresources (ResourceProvider, File/DataURI proxies, ImageAsset, MultiFrameImageAsset → codec)
sksg (render nodes, geometry, effects, invalidation; uses core + effects)
skottie core (Animation/Builder, Composition, layers, transforms, camera, animators/keyframes,
              property observers, slots)            → jsonreader, sksg, skresources
skottie effects (32 files; 9 build SkRuntimeEffects: BlackAndWhite, BrightnessContrast, Bulge,
                 DisplacementMap, FractalNoise, HueSaturation, SkSL, Sphere, Threshold) → effects, sksl
skottie text (TextAdapter, TextAnimator, RangeSelector, TextShaper → SkShaper factory/SkUnicode,
              falls back to the primitive shaper, TextShaper.cpp#L54-L66)   → shaper, unicode
```

`SkJSONReader` parses decimals with its own `pow10` table and falls back to `std::from_chars`.
Rust's `f32::from_str` is also correctly rounded, so the fallback is equivalent for every input
`from_chars` accepts. Its `std::pow` for exponents below −31 is a libm call (PORTING §5.7).

### 5.3 Tests

`modules/skottie/tests` (23): `Skottie_Shaper_CTStrict` is CoreText-only (`Shaper.cpp#L517-L520`)
and is excluded. `Skottie_Shaper_ExplicitFontMgr` is HarfBuzz-and-not-Windows (`#L268`) and runs.
The HAlign/VAlign tests carry "gross tolerances … for NativeFonts bots" (`Shaper.cpp`), ported as
they are. `Skottie_Expression*` use a fake `ExpressionManager` (`tests/Expression.cpp`). `SGTest` (2).
Benches: `DecodeBench.cpp` `SkottieDecodeBench`/`SkottiePictureDecodeBench`, 50 entries (32 filed
as `svg` because their file names contain "svgo", 18 as `core`/`text`/`effects`).

---

## 6. SVG (`modules/svg` 6.4k, `src/svg` 1.3k, `src/xml` 0.7k)

- **XML.** `SkXMLParser`/`SkDOM` port over a small safe tokenizer, as codecs.md Q4 recommends
  (`docs/design/codecs.md#L222`, task C13). Whichever of C13 and M12 runs first lands it in core
  (`xml::{parser, dom}`), plus `SkXMLWriter` for the SVG canvas.
- **DOM core first (M13).** It covers the elements and attributes `TestSVGTypeface` needs (decision
  10), and unblocks text T20: the 13 `gm/{coloremoji,coloremoji_blendmodes,scaledemoji*,fontmgr,
  mixedtextblobs}` and `Typeface_glyph_to_char` entries listed with `T20` reasons in the manifest.
- **Tests.**
  - `SVGDeviceTest` (19) and `Annotation_Svg*` (2) render through `SkSVGCanvas` and compare the XML
    (attributes, path data, `%g`-formatted transforms, `%.8g` positions). So M16 ports a
    C-`printf`-compatible `%g`/`%.Ng` formatter: shortest correctly rounded digits, then C's `%g`
    rules. MSVC's UCRT and glibc agree on these, so one implementation serves every host.
  - `modules/svg/tests` (2) check DOM behaviour and text positions.
  - `RecordOpts_MergeSvgOpacityAndFilterLayers` is a core record-optimization test, misfiled as `svg`.
- No SVG source goldens exist. DM's `svg` source needs `--svgs` and was not run.

---

## 7. PDF (`src/pdf`, 9.9k)

| Concern | What Skia does | What the tests check | Plan |
|---|---|---|---|
| Object serialization | `SkPDFTypes`, `SkPDFUtils::AppendScalar` → `SkFloatToDecimal` | exact strings (`PDFPrimitivesTest.cpp#L100-L271`), float round-trip (`#L386-L404`) | exact port; Skia's own float-to-decimal, no `printf` |
| Deflate | `SkDeflateWStream` over zlib `deflate` (level −1) | round-trip only (`PDFDeflateWStreamTest.cpp#L120-L178`) | `skia-rust-zlib::Deflate` (already a Chromium-zlib port, byte-exact against `oracle/codec-diff/zlib`) |
| JPEG | passthrough of the encoded bytes when decodable as YUV/gray; CMYK re-encoded | source bytes appear verbatim / CMYK does not (`PDFJpegEmbedTest.cpp#L65-L123`) | `SkPDF::JPEG::Decode/Encode` helpers on `skia-rust-libjpeg` |
| Metadata | XMP, `SkUUID` from MD5, creation date | substrings (`PDFDocumentTest.cpp#L168-L244`) | exact port (MD5 ported if core lacks it) |
| Fonts | Type3 for path-only/unembeddable (all portable typefaces), Type0/CIDFontType2 + `hb-subset` for TrueType, Type1 (FreeType-only metrics) | `ToUnicode` CMap text, `CanEmbedTypeface` | port all; subsetting per Q4 |
| Tagging, links, annotations | `SkPDFTag`, structure tree | substrings, structure (`PDFTagged*Test`) | exact port |

No PDF sink golden exists (the oracle did not run the `pdf` config), so whole-document bytes are
not compared today. Everything except `hb-subset` is pure Skia or an already-exact port.

---

## 8. Crates and layering

| Crate | Ports | Depends on |
|---|---|---|
| `skia-rust-unicode` | `modules/skunicode` (ICU path, client), ICU shim | core; `icu_segmenter`, `icu_properties` (+`unicode_bidi`), `icu_casemap`, `unicode-bidi`; later `skia-rust-unicode-data` (M4) |
| `skia-rust-shaper` | `modules/skshaper` (primitive, HarfBuzz-on-HarfRust, skunicode iterators, factory helpers) | core, unicode, `harfrust =0.9.0` |
| `skia-rust-paragraph` | `modules/skparagraph` | core, effects, shaper, unicode |
| `skia-rust-resources` | `modules/skresources` | core, codec |
| `skia-rust-sksg` | `modules/sksg` | core, effects |
| `skia-rust-skottie` | `modules/skottie`, `modules/jsonreader` (as `skottie::json`) | sksg, resources, shaper, unicode, effects, codec |
| `skia-rust-svg` | `modules/svg`, `src/svg` (SVG canvas) | core (`xml`), effects, resources, shaper |
| `skia-rust-pdf` | `src/pdf`, `include/docs/SkPDFDocument.h` | core, raster, effects, codec, libjpeg, zlib, text |

The module crates sit above `text`/`codec` and beside `gpu`; none depends on `gpu`. That refines
PLAN §3.1's line `gpu → shaper / unicode`. `tests/tools` (test-only) gains a dependency on
`skia-rust-svg` for `TestSVGTypeface`. Facade features as PLAN §3.2, plus `textlayout` (paragraph
+ shaper + unicode) to match skia-safe. Public paths follow skia-safe: `textlayout::*`, `Shaper`,
`shapers::{primitive, hb, unicode}`, `resources`, `skottie::Animation`, `svg::{Dom, Canvas}`,
`pdf::new_document` (`third_party/rust-skia/skia-safe/src/modules.rs`). Module tests go to
`tests/src/modules/<module>/…` (`xtask/src/verify.rs#L47-L68`). Module GMs
(`modules/<m>/gm/*.cpp`) are not yet mapped by `verify::module_path`; M0a adds the mapping.

---

## 9. Work breakdown

Sizes: S < 500 lines, M 500–1,500, L > 1,500 of Rust (excluding tests and generated data). C++
sizes in brackets. Every task is a `port/<name>` branch. Agents start on Haiku (PLAN §8.3); **(S)**
marks tasks that start on Sonnet and **(O)** tasks that start on Opus. Tasks are listed in
execution order. Within a wave, tasks are parallel unless "Depends" says otherwise.

### Wave M-0: bookkeeping and differential tools

| ID | Task | Sources | Depends | Size | Unlocks / targets |
|---|---|---|---|---|---|
| M0a | Manifest and harness. Exclude the 3 `dm/lottie/images/*.png` (not lottie sources), `Skottie_Shaper_CTStrict` (CoreText) and `SkUnicode_GetUtf8Words` (libgrapheme). Set the `needs-oracle` reason on the 187 lottie JSONs. Fix modules: 50 Skottie benches → `skottie`, 34 `ShaperBench` → `shaper`, `ParagraphBench`/`FuzzSkParagraph` → `paragraph`, `RecordOpts_MergeSvgOpacityAndFilterLayers` → `core`. Map `modules/<m>/gm/*.cpp` in `verify::module_path` and the GM registry | `xtask/src/{inventory,verify}.rs`, manifest | — | S | honest denominators |
| M0b **(S)** | `cargo xtask text-deps fetch` (HarfBuzz `9cb1fee5`, Chromium ICU `d578f2e8` `source/common` + `common/icudtl.dat`, `skparagraph` fonts per Q2) and `oracle/shaper-diff` with modes (a)–(c) of §2.4, expected files, replay test | §2.4 | — | M | the go/no-go evidence for M6 |
| M0c **(S)** | `oracle/unicode-diff`: every `SKICU_EMIT_FUNCS` call over the corpus. Line/word/char/sentence boundaries with **raw** rule status, bidi levels/direction/reorderVisual, the binary/int properties and `u_is*` over all code points, `u_strToUpper` per locale. UAX conformance files | §3.1 | — | M | the go/no-go evidence for M2/M4 |

### Wave M-A: Unicode

| ID | Task | Skia sources | Depends | Size | Unlocks / targets |
|---|---|---|---|---|---|
| M1 | `skia-rust-unicode`: `SkUnicode` API (`CodeUnitFlags`, `BidiRegion`, break and bidi iterator traits, UTF conversions), `SkUnicode.cpp`, `SkUnicode_hardcoded.cpp`, client implementation | `include/SkUnicode.h`, `src/SkUnicode{,_hardcoded,_client}.cpp` [0.8k] | — | M | `SkUnicode_Client` (1) |
| M2 **(O)** | `icu_shim` (30 functions with ICU semantics, rule-status emulation) + port of `SkUnicode_icu.cpp`, on ICU4X compiled data | `SkUnicode_icu.cpp`, `SkUnicode_icupriv.h` [0.8k] | M1, M0c | L | `SkUnicode_Compiled_{Native,GetSentences,Emoji,Ideographic}`, `SkUnicode_ICU4X_{Emoji,Ideographic}`, `SkUnicode_ToUpper`, `SkUnicode_ComputeCodeUnitFlags` (8) |
| M3 | Bidi: `SkUnicode_icu_bidi.cpp`, `SkBidiFactory_icu_full.cpp` over `unicode-bidi` + ICU4X data; `ubidi_reorderVisual` port | `SkUnicode_icu_bidi.cpp`, `SkBidiFactory_icu_full.cpp`, ICU `ubidiln.cpp` (reorderVisual) [0.3k] | M1 | M | `SkUnicode_GetBidiRegions{LTR,RTL,Mix1,Mix2}`, `SkUnicode_ReorderVisual` (5) |
| M4 **(O)** | Chromium break data for ICU4X: `icu4x-datagen` over the Chromium ICU tree with the `wordbrk`, `line_normal`, `lstm`, `khmer-dictbe`, `laodict` and `cjdict` changes, baked into `skia-rust-unicode-data`; regenerate on pin bumps | Chromium `deps/icu` `patches/`, `source/data/brkitr` | M2, M0c | M (+ data) | unicode-diff clean: words around `.`/`:`, SA and CJ line/word breaks |

### Wave M-B: shaper

| ID | Task | Skia sources | Depends | Size | Unlocks / targets |
|---|---|---|---|---|---|
| M5 | `skia-rust-shaper`: `SkShaper` API, run iterators (font-manager fallback, trivial, std language), primitive shaper, `SkShapers::Factory`, `FactoryHelpers` (`BestAvailable`) | `SkShaper.cpp`, `SkShaper_primitive.cpp`, `SkShaper_factory.cpp`, `utils/FactoryHelpers.h` [0.6k] | text T9 (`Font`), M1 | M | — (used by Skottie's fallback) |
| M6 **(S)** | HarfBuzz path on HarfRust: font funcs → `FontFuncs` with `skhb_*` arithmetic, face creation (sfnt or synthesized empty face), variations, 16.16 scale, `HBLockedFaceCache`, `LanguageBasedLineBreaker`, script iterator, the three shapers; `SkShaper_skunicode.cpp` | `SkShaper_harfbuzz.cpp` [1.7k], `SkShaper_skunicode.cpp` | M5, M2, M3, M0b | L | `ShaperTest` (36) |
| M7 | `ShaperBench` (34) | `bench/ShaperBench.cpp` | M6 | S | 34 bench entries (`ported`) |

### Wave M-C: paragraph

| ID | Task | Skia sources | Depends | Size | Unlocks / targets |
|---|---|---|---|---|---|
| M8 | Test fonts and collection: fetch and verify the `skparagraph` asset (Q2); `ResourceFontCollection`, `TestFontCollection`; `--paragraph_fonts`/`--norun…` semantics in the unit-test harness | `tests/SkParagraphTest.cpp#L74-L203`, `utils/TestFontCollection.cpp` | M0b, text T19b | S | — |
| M9 | Model: `TextStyle`, `ParagraphStyle`, `FontArguments`, `FontCollection`, `TypefaceFontProvider`, `ParagraphCache`, `ParagraphBuilderImpl`, `TextShadow`, `DartTypes`, `Metrics` | [≈2.5k] | M6 | M | — |
| M10 **(S)** | Layout and paint: `OneLineShaper`, `Run`/`Cluster`, `TextWrapper`, `TextLine`, `ParagraphImpl`, `Decorations`, `ParagraphPainterImpl` | [≈6k] | M9 | L | paragraph GMs (4 + 1 skip-match) |
| M11 | Test sweeps: `SkParagraphTest` in 6 batches of about 25 (150 asserting, 3 early-return), `SkShaperJSONWriterTest` (3, with its helper), `ParagraphBench` (2), `FuzzSkParagraph` | tests | M10, M8 | S each | 156 unit + benches |

### Wave M-D: SVG (M12–M13 can start immediately)

| ID | Task | Skia sources | Depends | Size | Unlocks / targets |
|---|---|---|---|---|---|
| M12 | XML: `SkXMLParser`, `SkDOM` (shared with codecs C13), `SkXMLWriter` | `src/xml/*` [0.7k] | — | M | — |
| M13 **(S)** | SVG DOM core: `SkSVGDOM`, node/container/transformable nodes, `svg`/`g`/`defs`/`use`, shapes, `SkSVGAttributeParser`, presentation attributes and `style`, `SkSVGRenderContext`, values and types | `modules/svg/src` [≈3.5k] | M12 | L | text T20 (13 entries), with `skia-rust-tools` |
| M14 | Paint servers and effects: gradients, patterns, `clipPath`, `mask`, `image` (resources), filters (`fe*`) | [≈2.5k] | M13, M18 | L | `Svg_Filters_NonePaintInputs` |
| M15 | SVG text (`SkSVGText`) | `SkSVGText.cpp` [0.7k] | M13, M6 | M | `Svg_Text_PosProvider` |
| M16 **(S)** | SVG canvas: `SkSVGCanvas`/`SkSVGDevice`, `%g`/`%.Ng` formatter | `src/svg/*` [1.3k] | M12, text T15a, codec PNG encode | L | `SVGDeviceTest` (19), `Annotation_Svg*` (2), fuzz `SVGCanvas` |

### Wave M-E: Skottie

| ID | Task | Skia sources | Depends | Size | Unlocks / targets |
|---|---|---|---|---|---|
| M17 | `SkJSONReader` (DOM, fast numbers, fallback) | `modules/jsonreader` [1.4k] | — | M | — |
| M18 | `skia-rust-resources`: providers, data-URI proxy, `ImageAsset`, `MultiFrameImageAsset`, external track | `modules/skresources` [0.8k] | codec | M | — |
| M19 | `skia-rust-sksg` | `modules/sksg` [4.7k] | effects | L | `SGInvalidation`, `SGMerge` (2) |
| M20 **(S)** | Skottie core: builder, composition, layers, transforms, camera, animators/keyframes, observers, slots, expression interface | `src`, `src/animator`, `src/layers` [≈8.5k] | M17–M19 | L | `Skottie_{Keyframe,Props*,Annotations,Layer_NoType,Gradient_InvalidCount,OssFuzz8956,AudioLayer,Expression*,Image_*}` |
| M21a/b/c | Effects: (a) color/blur/shadow/transform, (b) runtime-effect based (9), (c) layer styles, motion tile, displacement, corner pin | `src/effects` [6k] | M20, sksl | L each | — |
| M22 **(S)** | Skottie text: `Font`, `TextAdapter`, `TextAnimator`, `RangeSelector`, `TextShaper`, `TextValue`, `Unicode` | `src/text` [3.6k] | M20, M6 | L | `Skottie_Shaper_*` (7), `Skottie_Text_*` (3) |
| M23 | GMs (5), `SkottieSrc` port for `dm/lottie/*` (renders recorded; entries stay `todo`), `DecodeBench` (50) | `modules/skottie/gm/*`, `dm/DMSrcSink.cpp#L1293-L1373` | M21, M22 | M | 5 GMs; benches |

### Wave M-F: PDF (M24 can start immediately)

| ID | Task | Skia sources | Depends | Size | Unlocks / targets |
|---|---|---|---|---|---|
| M24 | Primitives and document: `SkPDFTypes`/`Union`, `SkPDFUtils` (incl. `SkFloatToDecimal`, `EmitPath`, `GetDateTime`), `SkDeflate`, `SkPDFMetadata`, `SkUUID`/MD5, document skeleton, page tree | [≈3k] | zlib | M | `SkPDF_Primitives{,_Scalar,_Color}`, `SkPDF_EmitPath`, `SkPDF_DeflateWStream`, `SkPDF_Metadata`, `SkPDFUtils_GetDateTime`, `SkPDF_document_tests`, `…skbug_4734`, `…pdfa_document`, `…unicode_metadata`, `…multiple_pages`, `…abort_jobs` |
| M25 **(S)** | Device: `SkPDFDevice`, graphic stack/state, resource dict, form XObjects, shaders (gradients, images), `SkPDFBitmap` (JPEG passthrough, deflated images), `SkKeyedImage`, `SkClusterator`, `SkPDFTag`, annotations | [≈5k] | M24, effects, codec, libjpeg | L | `SkPDF_JpegEmbedTest`, `…JpegIdentification`, `…OpaqueSrcModeToSrcOver`, `…ImageFilter`, `…Rasterize*` (2), `…GradientDegenerateStopsDoesNotCrash`, `…Clusterator`, `fuzz875632f0`, `SkPDF_tagged_*` (6), `Annotation_Pdf*` (2), `Canvas_pdf`, `canvas_clip_restriction`, `canvas_empty_clip` |
| M26 **(S)** | Fonts: `SkPDFFont` (Type3, Type0/CID), `MakeToUnicodeCmap`, `MakeCIDGlyphWidthsArray`, `SkPDFType1Font`, subsetting seam (Q4) | [≈2k] | M25, text | L | `SkPDF_ToUnicode`, `SkPDF_FontCanEmbedTypeface` |
| M27 | `PDFBench` (12), fuzz `PDFCanvas` | — | M26 | S | benches |

### Dependency summary

```
M0a   M0b ─────────────┐        M0c ──┬──────────┐
                       │              │          │
text T9 ─ M5 ─┐        │   M1 ─┬─ M2 ─┴─ M4      │
              └────────┴─ M6 ◄─┴─ M3             │
                           ├─ M7                 │
                           ├─ M9 ─ M10 ─ M11 ◄─ M8 (+text T19b)
                           ├─ M15, M22
M12 ─ M13 ─┬─ M14 (+M18)   └─ (paragraph, Skottie text, SVG text GMs)
           ├─ text T20
           └─ M16
M17, M18, M19 ─ M20 ─ M21a/b/c ─ M23 ◄─ M22
zlib ─ M24 ─ M25 ─ M26 ─ M27
```

Start immediately and in parallel: M0a, M0b, M0c, M1, M3, M12 → M13, M17, M18, M19, M24. The
critical path to the first module GM (`paragraph_`): M1 → M2 → M5/M6 (with M0b) → M9 → M10.

---

## 10. Risks and open questions

| # | Item | Plan |
|---|---|---|
| Q1 | **HarfRust `=0.9.0`** tracks HarfBuzz 14.2.0, not the oracle's 13.1.0, and is the newest release compatible with Skia's `read-fonts =0.40.1` | Approve, with the divergence policy of decision 5 (vendored patch crate `third_party/harfrust` only when shaper-diff shows a difference that Skia's callbacks don't absorb). The pin moves with Skia's Fontations pins on milestone bumps |
| Q2 | **Paragraph test fonts** (`skparagraph` asset v4: Roboto, Noto CJK/Emoji/Naskh, Source Han, Ahem, …; Apache-2.0 / OFL-1.1, tens of MB) | Proposed: don't commit them. Publish them once as a release asset `test-fonts-m156` (sha256 in `inventory/test-fonts.lock`), fetched and cached like the goldens. The PR that adds them lists the licences in `NOTICE`. Alternative: rebuild from `create.py`'s sources at test time (network and two git clones) |
| Q3 | **Exactness target for segmentation**: reproduce Chromium's patched ICU (M4), or stock ICU4X with documented differences | Recommended: reproduce. The oracle and Skia's bots used Chromium's data, and the `.` word-break change shows up in ordinary Latin text (200 of 909 corpus strings) |
| Q4 | **PDF font subsetting.** The default build uses `hb-subset`. The Rust subsetter (`skera`) needs skrifa ≥ 0.47 (two fontations versions in the tree) and does not produce HarfBuzz's bytes. No current test compares subset bytes | Options: (a) port Skia's `#else` branch (`SkPDFSubsetFont.cpp#L200-L210`: embed whole fonts) and record the deviation; (b) depend on `skera` with its own fontations versions behind the `pdf` feature; (c) port the `hb-subset` parts Skia uses (glyf/loca/cmap/hmtx/CFF; large). Recommended: (b), with the deviation recorded in `API_MAPPING.md` |
| Q5 | **C/C++ in CI for shaper-diff and unicode-diff** (HarfBuzz about 75 s, ICU `common` about 150 s on 4 cores) | Recommended: committed expected files replayed by `cargo test` everywhere (rp-diff model). Rebuild and compare only in a cached Linux job when a case or a pin changes |
| Q6 | **A Linux DM oracle for `--src lottie svg` and the `pdf`/`svg` sinks.** gpu.md Q1 already proposes CI-hosted oracle builds | If gpu.md Q1 is approved, add a CPU job: clang on Linux with the oracle's GN args, portable fonts. First check that it reproduces the existing RGBA-variant GM goldens byte for byte, then render the 187 lotties at those tiers. Until then they stay `needs-oracle` |
| Q7 | **Pull SVG core (M12–M13) ahead of the shaper**, to unblock text T20 | Recommended (no shaping dependency; 13 text entries plus the SVG module) |
| Q8 | **ICU4X version**: 2.3.x (pilot; CLDR 48.2.1, ICU 78.1rc data) or 2.1.x (CLDR 48.0) | Recommended: `=2.3.x`, chosen by unicode-diff results. M4's custom data makes the choice mostly about engines |
| Q9 | **Skottie expressions**: Skia ships only the `ExpressionManager` interface | Port the interface. No JavaScript engine (none exists at the pin) |
| R1 | HarfRust needs a `FontRef`, but the portable typeface has no sfnt | Synthesized minimal sfnt + `FontFuncs` (M6). shaper-diff mode (c) proves it matches the empty HarfBuzz face |
| R2 | ICU behaviour the callers can observe, beyond what the APIs document (inherited rule status, `u_isWhitespace`/`u_isspace` definitions, locale casing) | Shim tested call by call by unicode-diff (M0c, M2). Never "close enough" |
| R3 | Paragraph tests depend on Fontations metrics (text T19b) and real fonts | They pass on Skia's `NativeFonts_Fontations` bots with the same stack. If one fails, compare HarfRust and HarfBuzz on that string with shaper-diff before touching layout code |
| R4 | Hollow passes: paragraph tests without fonts, 3 tests that return early on the bots, `paragraph_layout_` skip-match, the excluded lotties | PR descriptions name them; the entries only count under the configuration in §4 |
| R5 | Text formatting: `%g`, `%.8g`, `SkFloatToDecimal`, JSON `from_chars` | Exact ports, no Rust `Display`. Unit-tested against C's `printf` outputs captured in the diff tools |
| R6 | libm calls (`std::pow` in `SkJSONReader`; trigonometry in Skottie transforms and effects) | `// skia-rust: libm` per PORTING §5.7. Mismatches traced to UCRT vs the host's libm are recorded and parked like the other libm cases (no `port/libm` dependency) |
| R7 | **Performance**: HarfRust is slower than HarfBuzz (its README says < 25 %; its own benchmark tables show up to about 3×) | `ShaperBench` entries stay `ported` (no bench goldens; bench.md C3). Perf parity needs upstream HarfRust work; track it, don't block on it |
| R8 | Scale: about 55k C++ lines across six modules | Split as above. M0 tools first, so Haiku tasks fail with a diff, not a mystery |

---

## Appendix A: reproducing the pilot (§2.3, §3.3)

- **HarfBuzz.** Download `harfbuzz.git/+archive/9cb1fee5…/src.tar.gz` from
  `chromium.googlesource.com/external/github.com/harfbuzz/harfbuzz` and compile `src/harfbuzz.cc`
  with Skia's defines (`-DHAVE_OT -DHAVE_CONFIG_OVERRIDE_H -DHB_NO_FALLBACK_SHAPE -DHB_NO_WIN1256`,
  Skia's `third_party/harfbuzz/config-override.h`, `-ffp-contract=off`). Link a 50-line driver:
  `hb_face_create`, `hb_font_create`, optional `hb_font_set_scale`,
  `HB_BUFFER_CLUSTER_LEVEL_MONOTONE_CHARACTERS`, `hb_buffer_guess_segment_properties`, `hb_shape`,
  and print `gid:cluster:xa:ya:xo:yo` per line. The Rust twin uses `harfrust =0.9.0`
  (`ShaperData::new`, `ShapeOptions::scale`, `UnicodeBuffer::set_cluster_level`).
- **ICU.** Download `chromium/deps/icu.git/+archive/d578f2e8…/source/common.tar.gz` and
  `source/stubdata`, and fetch `common/icudtl.dat`. Compile `common/*.cpp` with
  `-DU_COMMON_IMPLEMENTATION -DU_STATIC_IMPLEMENTATION -DU_DISABLE_RENAMING=1`. The driver calls
  `udata_setCommonData`, then `utext_openUTF8` + `ubrk_open(type, "en")` for the four break types
  (printing the raw `ubrk_getRuleStatus`), and `ubidi_setPara` + `ubidi_getLevelAt` for bidi.
  The Rust twin uses `icu_segmenter =2.3.0` (`new_auto`, `LineBreakStrictness`),
  `icu_properties =2.3.0` (`LineBreak` for hard breaks, `BidiClass` with feature `unicode_bidi`),
  and `unicode-bidi =0.3.18` (`BidiInfo::new_with_data_source`).
- These drivers are the seeds of M0b and M0c and were not committed with this note.

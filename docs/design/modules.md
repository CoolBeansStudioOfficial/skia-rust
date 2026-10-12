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

### 2.5 Results of M0b (2026-10-12)

`oracle/shaper-diff` implements §2.4: `cpp/shaper_diff.cc` (HarfBuzz `9cb1fee5`, built by `build.sh`
in about 80 s), the Rust twin (HarfRust `=0.9.0`), `configs.txt`, `fonts.txt` (the 36 sfnt fonts of
the paragraph test fonts), a corpus of 36 sources (the 34 `resources/text` scripts, 199 string
literals of the shaper, paragraph, Skottie and Unicode tests and the paragraph GM, and 63 text layers
of `resources/skottie`), and `expected/*.txt` (9,144 rows: config, font, source, line count, glyph
count, SHA-256 of the serialized result). `cargo test -p skia-rust-shaper-diff` replays them in about
15 s on any host; `cargo xtask oracle shaper-diff [--update|--replay]` rebuilds the C++ side. Every row
is exact or listed in `known-divergences.txt` with a class, and a listed row that starts matching
fails the test.

| Mode | Rows | Exact | Divergent |
|---|---|---|---|
| (a) built-in funcs, 3 scales | 3,888 | 3,760 (96.7 %) | 128 |
| (b) Skia's callbacks, 2 sizes × subpixel on/off | 5,184 | 5,030 (97.0 %) | 154 |
| (c) empty face, synthetic callbacks | 72 | 68 | 4 |

Five classes cover every divergent row; none is in a Latin, Greek, Cyrillic, Indic, Thai, CJK, Hebrew
or emoji source under Skia's callbacks (mode b):

1. **`fallback-mark-gap`** (mode b: 144 rows, Arabic and Syriac text in fonts without those marks).
   Root cause, which also explains the `.notdef` mark offsets of §2.3: HarfBuzz computes the gap for
   fallback mark positioning as `font->y_scale / 16` (`hb-ot-shape-fallback.cc#L220`); HarfRust 0.9.0
   as `scale_y(units_per_em / 16)` (`ot_shape_fallback.rs#L147`). The two differ by truncation of
   `units_per_em / 16` (394 of 786,432 at 12 pt in a 1000-upem font). Skia's callbacks do **not**
   absorb it. A one-line fix: a vendored patch (decision 5, option b).
2. **`builtin-extents`** (mode a only, 98 rows). The same fallback marks, but HarfRust's built-in glyph
   extents are missing where HarfBuzz has them (CFF fonts), so the offset is 0. Skia's `glyph_extents`
   callback replaces them, so this class is absorbed (decision 5, option a).
3. **`builtin-advance-rounding`** (mode a only, 26 rows). ±1 in a 16.16 advance of the built-in functions
   at 12 and 30 pt (CJK fonts, Cyrillic, Greek, Tamil, emoji, English). Absorbed by Skia's `advance`
   callbacks.
4. **`anchor-rounding`** (modes a and b, 14 rows: Katibeh and Noto Naskh Arabic, the fonts that really do
   have mark anchors). ±1 in the offset of an anchored mark. HarfBuzz 13.1 rounds the difference of the
   float-scaled anchors (`OT/Layout/GPOS/MarkArray.hh#L57`); HarfRust scales the font-unit difference
   (`ot/gpos/mark.rs#L33`). Not absorbed; a vendored patch (option b).
5. **`arabic-fallback-shaping`** (mode c, 4 rows: `text-arabic` and the test strings with Arabic).
   `arabic_fallback_shape` is a stub in HarfRust 0.9.0 (`ot_shaper_arabic.rs#L420`), so a face without
   Arabic GSUB gets no presentation forms. HarfBuzz builds fallback lookups
   (`hb-ot-shaper-arabic-fallback.hh`) when the font maps the presentation forms. This matters for the
   portable `TestTypeface`, which has no tables. Option c: port that file into the patch crate. With real
   fonts that lack the forms (mode b) it does not show, since no glyph exists to substitute.

The inputs of mode b are Fontations' unhinted advances and bounds (skrifa), as a stand-in for `Font`
(text T9); both sides read the same table, so the comparison does not depend on that choice. Mode c
uses synthetic callbacks (`src/synthetic.rs`), not `TestTypeface`'s numbers. Two things M6 still has to
add to the comparison once `Font` exists: the real callbacks' numbers, and variable-font coordinates.

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

M0c (§3.4) turned this pilot into a committed tool and found the raw-status, `following`/`preceding` and
bidi-direction differences the pilot did not look at.

### 3.4 Results of M0c (2026-10-12)

`oracle/unicode-diff` implements the measurement of §3.3 for every call of `SKICU_EMIT_FUNCS` that has an
ICU4X counterpart: `cpp/unicode_diff.cc` (Chromium ICU `d578f2e8`, `common/*.cpp` built by `build.sh` in
about 150 s), the Rust twin (ICU4X `=2.3.0`: `icu_segmenter`, `icu_properties`, `icu_casemap`;
`unicode-bidi =0.3.18` with ICU4X's `BidiClass`), `expected/*.txt`, `known-divergences.txt` and the replay
test. The corpus is 869 strings: the 34 `resources/text` files whole (as paragraphs, so their newlines
count), 600 string literals of the shaper, paragraph, Skottie and Unicode tests and the paragraph GM, 57
Skottie text layers and 178 hand-written cases. `cargo test -p skia-rust-unicode-diff` replays everything
in about 4 s on any host (the conformance test fetches the Unicode 17.0.0 UAX files like the test fonts);
`cargo xtask oracle unicode-diff [--update|--replay]` rebuilds the C++ side. Every row, code point range
and conformance line is exact or listed with a class, and a listed one that starts matching fails.

| Section (rows = strings) | Compared | Exact | Divergent |
|---|---|---|---|
| Character boundaries, UTF-8 and UTF-16 | 1,738 | 1,738 | 0 |
| Line boundaries with raw `ubrk_getRuleStatus` | 1,738 | 1,542 | 196 |
| Word boundaries with raw status | 1,738 | 1,224 | 514 |
| Sentence boundaries with raw status | 1,738 | 110 | 1,628 |
| `ubrk_following` / `ubrk_preceding` at every offset (line UTF-8, word UTF-16) | 1,738 | 1,307 | 431 |
| Bidi levels, direction, length (paragraph level LTR, RTL, default LTR, default RTL) | 3,476 | 3,399 | 77 |
| `ubidi_reorderVisual`, all level sequences of two alphabets | 25,749 | 25,749 | 0 |
| Binary and int properties, `u_iscntrl`, `u_isspace`, `u_isWhitespace`, script: 11 properties × 1,114,112 code points | 12,255,232 | all | 0 |
| `u_strToUpper`: root, `tr`, `az`, `lt`, `el` × 1,112,064 scalar values, and 4 locales × the corpus | 5,563,796 | all | 0 |

| Conformance file (Unicode 17.0.0) | Lines | ICU fails | ICU4X / unicode-bidi fails | Differ |
|---|---|---|---|---|
| `GraphemeBreakTest` | 766 | 0 | 0 | 0 |
| `WordBreakTest` | 1,944 | 17 | 0 | 17 |
| `SentenceBreakTest` | 512 | 0 | 0 | 0 |
| `LineBreakTest` | 19,338 | 142 | 460 | 318 (the 142 are failed by both) |
| `BidiCharacterTest` | 91,707 | 31 | 0 | 31 |

Four things the pilot (§3.3) did not show:

- **Character boundaries, properties, case mapping and `reorderVisual` are exact.** So are the
  `u_isspace`/`u_isWhitespace`/`u_iscntrl` definitions of `uchar.cpp` over the General_Category, and
  ICU4X's script and `Line_Break` values after `to_icu4c_value()` (deprecated upstream, icu4x#6067; M2 needs
  the same table).
- **Sentence and word boundary *positions* are exact except the data classes below; the raw rule status
  is not.** ICU4X has no status at all for lines and sentences, and `WordType` (None, Number, Letter) for
  words. The status differences are the bulk of the divergent rows.
- **`BidiInfo::levels` has no rule L1.** ICU's `ubidi_getLevelAt` does (separators and trailing white space
  at the paragraph level); the twin uses `reordered_levels_per_char` over the paragraph. With that,
  `unicode-bidi` passes all 91,707 `BidiCharacterTest` lines; ICU fails 31 (embeddings mixed with
  isolates), so ICU is the less conformant side there.
- **`ubidi_getDirection` and the explicit-embedding levels follow ICU's own shortcuts** (`ubidi.cpp#L1007-L1020`),
  which the corpus strings with U+202A-202E hit.

Divergence classes (all 3,212 are classified; `unclassified` is rejected by the replay test). "Strings" are
distinct corpus strings; "rows" count every case (UTF-8 and UTF-16, the four paragraph levels, navigation)
separately; "places" are the differing boundaries, levels or lines, counted once (UTF-8 cases only).

| Class | Strings | Rows | Places | What it is, and what fixes it |
|---|---|---|---|---|
| `word-dot` | 192 | 576 | 394 | `wordbrk.patch` (`.`, U+FF0E: MidNumLet → MidNum). **M4** (Chromium break data) |
| `word-colon` | 3 | 26 | 23 | `wordbrk.patch` (`:` out of MidLetter); also 17 `WordBreakTest` lines (rule 6.0). **M4** |
| `word-hangul-syllable-jamo` | 1 | 3 | 1 | ICU splits U+AC00 from a following U+11A8; UAX #29 does not. **M4**, to confirm against the rule sources |
| `word-dictionary-cj` | 8 | 24 | 33 | cjdict vs ICU4X's dictionary. **M4** |
| `word-dictionary-sa` | 10 | 30 | 164 | Thai, Lao, Khmer, Myanmar, Tai Tham, New Tai Lue: ICU dictionaries vs ICU4X LSTM. **M4** |
| `line-dictionary-sa` | 13 | 39 | 456 | the same for line breaks. **M4** |
| `line-normal-other` | 1 | 3 | 1 | a break after U+FE13 that ICU makes and ICU4X Normal does not. **M4** |
| `icu4x-lb-<rule>` (15 rules) | - | 318 | 318 | `LineBreakTest` lines ICU passes and ICU4X fails: rule 9.0 (139), 999.0 (60), 28.0 (50), 20.1 (42), and 11 more rules (1-4 lines each). Not root-caused beyond the rule; ICU4X's line data. **M4** |
| `word-status-trailing-extend` | 22 | 44 | 625 | ICU status 200, ICU4X `WordType::None` for a segment ending in a mark or Extend/Format. **M2** (compute the status from the segment) |
| `word-status-ideographic` | 28 | 56 | 606 | ICU 400 (`UBRK_WORD_IDEO`), ICU4X Letter. **M2** |
| `word-status-none-vs-letter` | 2 | 4 | 21 | ICU 0, ICU4X Letter (after U+203F before U+FF65; regional indicator before an emoji). **M2** |
| `sentence-status-sep` | 814 | 1,628 | 926 | ICU 100 (`UBRK_SENTENCE_SEP`) for a sentence that ends at a paragraph separator or at the end of text without a terminator, 0 after `.?!`. **M2** |
| `line-status-hard-break` | 93 | 186 | 237 | ICU 100 (`UBRK_LINE_HARD`) after LF, CR, VT, FF, NEL, LS, PS. **M2** |
| `line-status-dictionary-hard` | 5 | 10 | 48 | ICU 100 for dictionary boundaries in a range that ends in a hard break (§3.2's Lao quirk). **M2** |
| `nav-offset-inside-code-point` | 190 | 218 | - | `ubrk_preceding(i)` for an `i` inside a UTF-8 sequence or surrogate pair: ICU first moves `i` to the code point's start. **M2** |
| `bidi-direction` | 30 | 47 | 47 | `ubidi_getDirection` is `directionFromFlags` (text content and paragraph level), not the levels. **M3** |
| `bidi-uniform-direction-shortcut` | 11 | 36 | 67 | ICU does not resolve text without the opposite direction: embeddings and isolates in it have no effect. **M3** |
| `bidi-x9-removed-levels` | 5 | 20 | 52 | levels of LRE/RLE/LRO/RLO/PDF/BN: ICU gives an opener the level it opens and a closer the outer level. **M3** |
| `bidi-default-level-number-after-separator` | 1 | 1 | 1 | `UBIDI_DEFAULT_RTL`: a digit after an empty paragraph gets level 2 in ICU (`text-sinhala`). **M3** |
| `icu-bidi-conformance` | - | 31 | 31 | `BidiCharacterTest` lines only ICU fails. Nothing to do: Skia would reproduce them only by using ICU's algorithm (M3 decides whether to emulate) |

What this means for the tasks. **M2** has a precise list of what the shim must compute that ICU4X does not
give: word status (None 0, Number 100, Letter 200, Kana 300, Ideo 400 from the segment's characters),
line status 100 after hard breaks and for the dictionary range quirk, sentence status 100 after
separators or the end of text, `following`/`preceding` at offsets inside code points, and the numeric tables. **M3** has the
`directionFromFlags` shortcut, the removed-character levels and L1 over `unicode-bidi`. **M4** has the
data classes: 192 strings with `word-dot`, the dictionaries and the rest; unicode-diff is clean for
them exactly when the category rows in the table above disappear. Moving a class into the shim or the
data deletes its rows from `known-divergences.txt` (the replay test fails until it does), so the list
is the progress meter of M2, M3 and M4.

### 3.5 Results of M2 (2026-10-12)

`skia_rust_unicode::icu_shim` implements the `SKICU_EMIT_FUNCS` functions other than `ubidi_*` (M3) with
ICU's contracts, and `unicode_icu` ports `SkUnicode_icu.cpp` on top of it (with M3's `icu_bidi`). The
Rust twin of `oracle/unicode-diff` now calls the shim for every break, property, case and locale call, so
unicode-diff measures the shim. What the shim adds to ICU4X:

- **Rule status.** Lines: `UBRK_LINE_HARD` when the rule-based range ends in `BK`/`CR`/`LF`/`NL`
  (`line_normal.txt#L130-L137`). Sentences: `UBRK_SENTENCE_SEP` for the one tagged rule of `sent.txt`
  (`{bof}` is the start of each match: `handleNext` starts every match with the "beginning" category).
  Words: the tags depend on more than the final character (13a/13b, `$KanaKanji $KanaKanji`, Hangul,
  `^` rules), so `icu_shim::word_rules` runs the rule-based pass of Chromium's `word.txt` as RBBI does:
  `!!chain`ed rules, the match ends at the last accepting position, the status is the largest tag
  accepting there.
- **Dictionary ranges.** A boundary the rule-based pass does not have was added by a dictionary and takes
  the status of the rule-based boundary that ends its range (`populateDictionary`,
  `fOtherRuleStatusIndex`); this is §3.2's Lao quirk. For lines, the rule-based pass is ICU4X's
  rules-only segmenter over the text with LB1 applied as ICU's rules do (each `SA` character replaced by an
  `AL`, or a `CM` for `Mn`/`Mc`, of the same encoded length). ICU4X's own rules-only mode ends a range at
  every edge of a complex-script run, which ICU's rules do not.
- **Navigation.** `following` / `preceding` move an offset inside a code point to its start
  (`utext_setNativeIndex`) and follow `BreakCache`'s `fDone` handling.
- **Locales.** `ubrk_open`'s rule file per locale from the `boundaries` tables of Chromium's `brkitr`
  bundles (root, ja, ko, zh, zh_Hant, el) and the `lb` / `lw` keywords; `line_cj.brk` (`ko@lb=strict`) and
  `title.brk` are named but absent, so opening them fails with `U_INVALID_FORMAT_ERROR` as in ICU. The
  `_phrase_cj` files (ICU's ML phrase breaking, `@lw=phrase`) run the `_cj` rules of the same strictness:
  ICU4X has no counterpart, and no Skia caller asks for it. `u_strToUpper` picks the case rules with
  `ucase_getCaseLocale`. `uloc_forLanguageTag` handles language, extlang, script, region, variants and
  the extensions (key and type names for the common `-u-` keys only). `uloc_getDefault` is `en_US`.
- **The data seam.** `icu_shim::BreakDataSource` hands out the ICU4X segmenters (per rule file); M4's
  Chromium break data implements it, and `UBreakIterator::open_with_data` takes it.

unicode-diff gains the `misc` section (`u_errorName`, `uloc_forLanguageTag`, `uloc_getDefault`, now
compared) and 7 `breaks` cases that open other locales (`zh_Hans_CN`, `ja_JP`, `ja@lb=normal`,
`en@lb=strict`, `en_US@lb=loose`, `ko@lb=strict`, `el_GR`).

| Class (M0c rows) | Before | After |
|---|---|---|
| `word-status-trailing-extend` | 44 | 0 |
| `word-status-ideographic` | 56 | 0 |
| `word-status-none-vs-letter` | 4 | 0 |
| `sentence-status-sep` | 1,628 | 0 |
| `line-status-hard-break` | 186 | 0 |
| `line-status-dictionary-hard` | 10 | 0 |
| `nav-offset-inside-code-point` | 218 | 0 |

Divergent `breaks` rows of the M0c cases: line 196 → 28, word 514 → 428, sentence 1,628 → 0, navigation
431 → 228; all that is left there is M4's data classes (`word-dot`, `word-colon`, `word-dictionary-*`,
`word-hangul-syllable-jamo`, `line-dictionary-sa`, `line-normal-other`), with the same strings and places
as in §3.4. The locale cases add 70 rows, all `line-dictionary-sa` and `line-normal-other` on the same 14
strings; `ko@lb=strict` and `el_GR` are exact. `known-divergences.txt`: 3,212 → 1,197 entries. Props, case,
reorder and misc are exact; bidi and conformance are unchanged (M3, M4).

Beyond the corpus, three random corpora of 16,000 strings over the characters the rules single out
(letters, digits, `ExtendNumLet`, quotes, Hebrew, kana, kanji, iteration marks, Hangul and jamo, the SA
scripts with their marks, emoji and modifiers, regional indicators, every hard break, CR LF, terminators,
closers, format and extend characters) were run through both sides (`$SKIA_RUST_UNICODE_DIFF_CORPUS` and
the driver's `--corpus`): at every boundary both sides have, the status is ICU's, except one string where a
`line-normal-other` break (after U+FE13) splits ICU's range, and navigation is exact wherever the
boundaries are.

### 3.6 Results of M4 (2026-10-12)

M4 was planned as `icu4x-datagen` over Chromium's ICU tree. That does not work, for two reasons, so M4
reproduces Chromium's segmentation with ICU's own run time and ICU's own compiled data instead:

- **ICU4X's data cannot express Chromium's rules.** ICU4X 2.3's rule-based segmenter data comes from
  `icu_provider_source`'s own TOML rule files, compiled into the crate (`src/segmenter/mod.rs#L941-L1037`:
  `include_str!("../../data/segmenter/{line,word,sentence,grapheme}.toml")`). There is no input for ICU's
  RBBI rule files (`source/data/brkitr/rules/*.txt`), which are what Chromium patches (`wordbrk.patch`,
  `line_normal.patch`); translating them to ICU4X's rule language would be invention, not a port.
- **Most of the classes are engines, not data.** ICU splits dictionary ranges with `dictbe.cpp`:
  `ThaiBreakEngine` and its Lao, Burmese and Khmer siblings look three words ahead, resynchronise after
  non-words, never stop before a mark and handle Thai suffixes; `CjkBreakEngine` normalises to NFKC and
  runs a Viterbi search over the dictionary's costs with a katakana-run heuristic. ICU4X has an LSTM and
  a different dictionary algorithm. The 318 `icu4x-lb-<rule>` lines are ICU4X's UAX #14 engine against
  ICU's `line_normal` rules. Chromium also changes engine code (`khmer-dictbe.patch`: the Khmer
  thresholds 10 and 5).

What M4 adds:

- **`skia-rust-unicode-data`**: the 17 `brkitr` files of Chromium's `common/icudtl.dat` at the pin (12
  `.brk` rule tables, 5 `.dict` dictionaries; `title.brk` and `line_cj.brk` are not in Chromium's data),
  byte for byte, with their SHA-256 and ICU's `LICENSE`. They are what Chromium's ICU loads, built from
  `source/data/brkitr` with every patch applied (`wordbrk`, `line_normal`, `lstm`: no LSTM models, so Thai
  and Burmese use dictionaries, `cjdict`). 3,231,904 bytes (cjdict 2.0 MB, burmese 254 kB, thai 126 kB,
  khmer 94 kB, lao 52 kB, line tables 73-86 kB each, word 23 kB, sentence 20 kB each, char 14 kB). They stay
  in ICU's binary form (`include_bytes!`, read in place, little-endian on every host): that is the compact
  form; the same tables as Rust source would be several times larger. `std` only, no `unsafe`, no code
  besides the lookup functions.
- **`cargo xtask text-deps break-data [--check]`** regenerates the crate from `icudtl.dat` (TOC of ICU's
  common data package, `brkitr/*.brk|*.dict`) after an ICU pin bump; `--check` fails when the committed
  crate differs.
- **A port of ICU's break-iterator run time in `icu_shim`**: `rbbidata.cpp` (sections of a `.brk`),
  `ucptrie.cpp` (the read side of the category trie), `rbbi.cpp` (`handleNext`, `handleSafePrevious`,
  `first`/`last`/`next`/`previous`/`following`/`preceding`/`isBoundary`/`getRuleStatus`, the engine lookup
  of `getLanguageBreakEngine`), `rbbi_cache.cpp` (`BreakCache` with its 128-entry ring and `populateNear` /
  `populateFollowing` / `populatePreceding`, `DictionaryCache`), `brkeng.cpp` (`UnhandledEngine`,
  `loadEngineFor` with the root bundle's `dictionaries` table), `dictbe.cpp` (the five engines, with
  `khmer-dictbe.patch`), `dictionarydata.cpp` and the read side of `ucharstrie.cpp` / `bytestrie.cpp`, over a
  `UText` with ICU's native indexing. `CjkBreakEngine`'s NFKC uses ICU4X's normalizer (`icu_normalizer
  =2.3.0`); `hasBoundaryBefore` is the `ccc` of the first NFKD character, which equals ICU's on the
  engine's character set (a test checks every character).
- **The seam**: `BreakDataSource::rule_file` / `dictionary` hand out ICU data; `ChromiumData` (the crate)
  is the default of `UBreakIterator::open`, so of `SkUnicodes::ICU::Make`. `CompiledData` (ICU4X with M2's
  emulation) stays available through `open_with_data`.

Not ported, both outside Skia's calls: phrase breaking (`@lw=phrase` selects the `_phrase_cj` rules, which
run, but without ICU's ML phrase engine and the `ja` skip set), and one piece of process-wide state. ICU's
`ICULanguageBreakFactory` keeps the engines it has created for the life of the process and hands any of them
to a character its set holds, before it creates one by the character's script; the shim behaves as a
process that has created every engine. The difference shows only for the four Common-script characters of
the Chinese/Japanese set (U+30FC, U+FF70, U+FF9E, U+FF9F) at the start of a dictionary range, in a process
that has not yet met a Han, Hiragana or Katakana dictionary character.

| Class (rows) | After M2 | After M4 | What fixed it |
|---|---|---|---|
| `word-dot` | 576 | 0 | `word.brk` with `wordbrk.patch` |
| `word-colon` (incl. 17 `WordBreakTest` lines) | 26 | 0 | the same; the twin now fails ICU's 17 lines (rule 6.0, `a:a`) |
| `word-hangul-syllable-jamo` | 3 | 0 | ICU's rules, not Chromium's: `word.txt#L70-L79` keeps Hangul syllables out of `$ALetterPlus` (they are dictionary characters, `$dictionaryCJK`) and joins them only to each other (`#L111`, `#L175`); with no Korean dictionary, U+11A8 after U+AC00 starts a word |
| `word-dictionary-cj` | 24 | 0 | `cjdict.dict` + `CjkBreakEngine` |
| `word-dictionary-sa` | 30 | 0 | the four dictionaries + their engines; Tai Tham and New Tai Lue go to `UnhandledEngine` |
| `line-dictionary-sa` | 104 | 0 | the same, for lines |
| `line-normal-other` | 8 | 0 | `line_normal.brk` |
| `icu4x-lb-<rule>` (15 rules, `LineBreakTest`) | 318 | 0 | ICU's `line_normal` rules: the twin now fails exactly ICU's 142 lines (decided: Chromium's data, so ICU's side) |

`known-divergences.txt`: 1,197 → 108 entries, all bidi (77 rows of M3's classes and the 31
`icu-bidi-conformance` lines). Every `breaks` row (14,773: line, word, character and sentence boundaries with
rule status in UTF-8 and UTF-16, the 7 locale cases, `following`/`preceding` at every offset) is exact. Beyond
the corpus, three random corpora (2,000 strings of up to 30 tokens, 300 of up to 400 tokens, which overflow
the 128-entry cache and send `following`/`preceding` through `handleSafePrevious`, and 5,000 of up to 12
tokens; tokens are Thai, Lao, Khmer, Myanmar, Tai Tham, New Tai Lue and Tai Viet words and marks, kanji,
kana, half-width kana and sound marks, CJK compatibility ideographs and supplementary Han, Hangul syllables
and jamo, `.`/`:` and their full-width forms, separators, controls and emoji) give 124,100 rows through the
driver's `--corpus` and `$SKIA_RUST_UNICODE_DIFF_CORPUS`: all exact.

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
| Fonts | Type3 for path-only/unembeddable (all portable typefaces), Type0/CIDFontType2 + `hb-subset` for TrueType, Type1 (FreeType-only metrics) | `ToUnicode` CMap text, `CanEmbedTypeface` | port all; subsetting by `skia-rust-hb-subset` (Q4, §7.1) |
| Tagging, links, annotations | `SkPDFTag`, structure tree | substrings, structure (`PDFTagged*Test`) | exact port |

No PDF sink golden exists (the oracle did not run the `pdf` config), so whole-document bytes are
not compared today. Everything except `hb-subset` is pure Skia or an already-exact port; `hb-subset` is ported in §7.1.


### 7.1 Font subsetting: `skia-rust-hb-subset`

**Seam.** `SkPDFSubsetFont.cpp#L141-L217` (`subset_harfbuzz`) makes one `hb-subset` call: face from the typeface's stream
(`hb_face_create` on the whole file, `ttc_index`), `hb_subset_input_create_or_fail`, the used glyph ids in
`HB_SUBSET_SETS_GLYPH_INDEX`, flags `HB_SUBSET_FLAGS_RETAIN_GIDS` plus `HB_SUBSET_FLAGS_NOTDEF_OUTLINE` when glyph 0 is
used, `hb_subset_or_fail`, `hb_face_reference_blob`. Every other input set keeps its default. The crate exposes that call:
`skia_rust_hb_subset::subset_font(font_data, glyph_ids, ttc_index) -> Option<Vec<u8>>` (and `try_subset_font`, which also
says whether `None` is a HarfBuzz failure or a part that is not ported). `skia-rust-pdf` (M26) calls it; the PDF crate is not
touched by the port.

**What is reachable, decided from the source.** `SkPDFFont::FontType` (`SkPDFFont.cpp#L333-L355`) sends variable fonts
(`kVariable_FontFlag`, any `fvar`), `kCFF_Font` (FreeType's "CFF", also for OpenType-CFF and CFF2), non-standard containers and
unembeddable fonts to Type3, and only `kTrueType_Font` reaches `SkPDFSubsetFont` (`emit_subset_type0`, `#L468-L473`);
`kType1CID_Font` embeds the original data. So the subsetter sees non-variable TrueType-outline (`glyf`) fonts, with any of the
layout, colour and metadata tables such a font can have.

**Layers.** `serialize` is `hb_serialize_context_t` (objects packed from the tail, shared by content, links, snapshot/revert,
discard); `sfnt` is `hb_face_builder` (tables sorted by size then tag, 4-byte padding, `head.checkSumAdjustment`); `plan` is
`hb_subset_plan_t` (cmap-based glyph set, GSUB closure, composite closure, `RETAIN_GIDS` maps, name ids, MATH/COLR closures);
one module per table family (`glyf`, `cmap`, `tables` = maxp/head/post/OS/2/hmtx/vmtx/hdmx/name, `gdef`, `gsubgpos` + `gsub` +
`gpos` + `context`, `base`, `math`, `colr` + CPAL, `color` = sbix, `cbdt` = CBLC/CBDT, `var` = gvar). Reads use `View`, a byte
slice with HarfBuzz's `Null` semantics (a read outside the data is 0); sets and maps are `BTreeSet`/`HashMap`. Fontations
crates are not used: they would not produce HarfBuzz's bytes.

**Ported (byte-exact against the pin).** Everything the table above names for TrueType fonts: `glyf`/`loca`/`head`, `cmap`
(formats 4 and 12 output; 0/4/6/10/12/13 input), `hmtx`/`hhea`, `vmtx`/`vhea`, `maxp`, `post`, `name`, `OS/2` (including the
unicode range bits), `hdmx`, `GDEF`, `GSUB`/`GPOS` (all lookup types and formats with 16-bit offsets, extension, feature,
script and language-system pruning, duplicate-feature merging, GSUB closure), `BASE`, `MATH`, `COLR` (v0 and v1),
`CPAL`, `sbix`, `CBLC`/`CBDT`, `gvar`, name-id closure from `STAT`/`fvar`/`CPAL`/layout features, passthrough of
`STAT`/`cvt `/`fpgm`/`prep`/`fvar`/`avar`/`cvar`/`MVAR`, the drop list (`DSIG`, `kern`, `morx`, `SVG `, ...), the serialization
buffer growth of `_hb_subset_table` (a table that needs more than 256 times its source fails, as in HarfBuzz), and
`hb-repacker` (`graph`, `graph_gsubgpos`, `repacker`, `hbmap`): graph sorting by shortest distance, 24/32-bit space
assignment and subgraph isolation, duplication of shared nodes, priority raising, `GSUB`/`GPOS` extension promotion, and the
splitting of `PairPos` formats 1 and 2, `MarkBasePos` format 1 and `LigatureSubst` subtables. `hb_hashmap_t`'s open addressing is
copied (`HbMap`) because the repacker visits nodes in its iteration order.

**Not ported, with the reason.**
- `CFF `, `CFF2`, `VORG`, and `HVAR`/`VVAR`: unreachable from Skia (above). They report `Unsupported`.
- Variation data inside layout and colour tables (`GDEF` `ItemVariationStore`, `GSUB`/`GPOS` `FeatureVariations`, `BASE` and
  `COLR` variation stores): variable fonts only, so unreachable. `Unsupported`.
- The HarfBuzz sanitizer. A malformed table makes `hb-subset` fail (`FAIL` in the oracle); this port subsets what it can read
  and applies size checks only. Fonts that sanitize (every font in the corpus) are unaffected.
- `hb-repacker` pieces that no `GSUB`/`GPOS` the corpus can build reaches: the `GPOS` splitters for `PairPos` with device
  tables inside value records are ported but only the device-table-free path is exercised by the oracle fonts.

**Verification.** `oracle/subset-diff/` builds HarfBuzz at the pin with Skia's defines (`build.sh`), runs a C driver that calls
`hb-subset` exactly as `SkPDFSubsetFont.cpp` does, over `corpus.txt` (every Skia resources font with outlines: single glyph,
last glyph, ASCII, three seeded random sets, all glyphs, each with and without glyph 0) and records `expected.txt` (length and
SHA-256 per entry). `crates/skia-rust-hb-subset/tests/subset_diff.rs` replays the corpus and requires identical bytes: no
tolerance, and a font that is not ported must report `Unsupported` for the part it stops at (the test fails when a listed font
becomes exact, so the list can only shrink). The same test accepts `HB_SUBSET_EXT_DIR` for a larger local corpus made by the same
scripts (HarfBuzz's own test fonts and system fonts are not redistributable here). 5672 entries of such a local corpus
(HarfBuzz's test fonts and system fonts) are exact; what does not match is two fonts that HarfBuzz rejects in its sanitizer
(`FAIL` in the oracle) and the entries that need a part listed above.

Real fonts hardly ever overflow in ways that reach the repacker's extension promotion and subtable splitting, so
`oracle/subset-diff/gen_synthetic.py` writes synthetic fonts (`oracle/subset-diff/synthetic/`, replayed by
`repacker_matches_hb_subset_on_synthetic_fonts`) whose `GSUB`/`GPOS` need them: ligature, mark-to-base and pair-positioning
subtables that only fit after splitting, plain lookups promoted to extensions, shared nodes duplicated, and two `PairPos`
format 2 fonts for which HarfBuzz itself fails (its split leaves the first half just over the limit); the port fails with it.

---

## 8. Crates and layering

| Crate | Ports | Depends on |
|---|---|---|
| `skia-rust-unicode` | `modules/skunicode` (ICU path, client), ICU shim | core; `icu_segmenter`, `icu_properties` (+`unicode_bidi`), `icu_casemap`, `unicode-bidi`; later `skia-rust-unicode-data` (M4) |
| `skia-rust-shaper` | `modules/skshaper` (primitive, HarfBuzz-on-HarfRust, skunicode iterators, factory helpers) | core, unicode, `harfrust =0.9.0` |
| `skia-rust-paragraph` | `modules/skparagraph` | core, effects, shaper, unicode |
| `skia-rust-hb-subset` | `hb-subset` as Skia's PDF backend uses it (§7.1) | `std` only |
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
| M4 **(O)** | Chromium break data: the `brkitr` rule tables and dictionaries of Chromium's `icudtl.dat` (`wordbrk`, `line_normal`, `lstm`, `cjdict` applied) in `skia-rust-unicode-data`, regenerated on pin bumps; ICU's break-iterator run time and dictionary engines (with `khmer-dictbe`) ported into `icu_shim` (planned as `icu4x-datagen`; why not: §3.6) | Chromium `deps/icu` `patches/`, `source/data/brkitr`, `rbbi*.cpp`, `dictbe.cpp` | M2, M0c | M (+ data) | unicode-diff clean: words around `.`/`:`, SA and CJ line/word breaks |

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
| Q4 | **PDF font subsetting.** The default build uses `hb-subset`. The Rust subsetter (`skera`) needs skrifa ≥ 0.47 (two fontations versions in the tree) and does not produce HarfBuzz's bytes. No current test compares subset bytes | **Decided: (c).** Port the `hb-subset` parts Skia's PDF backend uses, byte-exact, in `skia-rust-hb-subset` (§7.1): `std` only, no `unsafe`, verified against HarfBuzz at the pin by `oracle/subset-diff`. The options were (a) port Skia's `#else` branch (`SkPDFSubsetFont.cpp#L200-L210`: embed whole fonts) and record the deviation, (b) depend on `skera`, (c) this. (c) is what the task asked for and makes the PDF font streams match Skia's; (a) and (b) would not. What is not ported is listed in §7.1 |
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

### Decided (2026-10-12)

The maintainer decided the open questions as follows. They bind the tasks of §9.

- **Q1: approved.** HarfRust `=0.9.0`, with the divergence policy of decision 5. M0b's findings (§2.5)
  say which divergences need a vendored patch (`third_party/harfrust`): `fallback-mark-gap` and
  `anchor-rounding` (small), and `arabic-fallback-shaping` (a port of `hb-ot-shaper-arabic-fallback.hh`).
- **Q2: published once as a release asset.** `test-fonts-m156` (SHA-256 in
  `inventory/test-fonts.lock`), fetched and cached like the goldens (`oracle/text-deps`,
  `cargo xtask text-deps fetch`); the licences are in `NOTICE`; `tools/test-fonts/build.sh` rebuilds the
  archive from the sources of Skia's `create.py`. One deviation from the bots' asset, for the
  maintainer to confirm: `GoogleSans-Regular.ttf` is **not** included, because the font's own licence
  string says it is not an open-source font. Only `SkParagraph_Intrinsic` uses the family. It is listed
  under `[excluded]` in the lock with its SHA-256.
- **Q3: reproduce Chromium's patched ICU segmentation (M4).**
- **Q5: committed expected files, replayed by `cargo test`.** C/C++ is rebuilt only in a cached job
  (`cargo xtask oracle shaper-diff`; HarfBuzz about 80 s here).
- **Q8: ICU4X `=2.3.x`.**
- **Q9: Skottie expressions: port the interface only.** No JavaScript engine.

### Status of M0c (2026-10-12)

M0c is done: `oracle/unicode-diff` (C++ driver over Chromium ICU `d578f2e8`, Rust twin over ICU4X `=2.3.0` and
`unicode-bidi =0.3.18`, expected files, `known-divergences.txt` with every divergence classified, replay
test), `cargo xtask text-deps fetch --only ucd` (the Unicode 17.0.0 conformance files, digests in
`inventory/text-deps.lock`) and `cargo xtask oracle unicode-diff [--update|--replay]`. Results and the class
table are in §3.4. The locale argument of `ubrk_open` is fixed at `en`: tailoring by client locale (for example `ja` or `zh` line
breaks) is not varied and is for M2's cases to add.

### Status of M2 (2026-10-12)

M2 is done (§3.5): `skia_rust_unicode::icu_shim` and `skia_rust_unicode::unicode_icu` (`SkUnicodes::ICU::Make`;
`SkUnicodes::ICU4X::Make` is the same implementation), the unicode-diff twin running through the shim with
every M2 class gone, and the eight `SkUnicode` tests of M2's row (plus M3's five bidi tests switched from
their stand-in to the ICU implementation). `SkUnicode_icu.cpp` uses M3's `icu_bidi` (merged from
`port/text-m3-m5`). Not ported: `SkIcuBreakIteratorCache`'s de-duplication by actual locale and its purge of
live iterators (memory only).

### Status of M4 (2026-10-12)

M4 is done (§3.6): `skia-rust-unicode-data` (Chromium's compiled break data, `cargo xtask text-deps
break-data`), the port of ICU's `RuleBasedBreakIterator`, caches and dictionary engines in `icu_shim`, and
`ChromiumData` as the default break data. unicode-diff's `breaks` section and the break conformance files are
exact; only bidi remains in `known-divergences.txt`.

### Status of M6 (2026-10-12)

M6 is done: `skia_rust_shaper::harfbuzz` (`SkShaper_harfbuzz.cpp`: `ShaperDrivenWrapper`, `ShapeThenWrap`,
`ShapeDontWrapOrReorder`, `LanguageBasedLineBreaker`, the script run iterator, the `skhb_*` font functions as
HarfRust `FontFuncs`, the face from the typeface's sfnt or, for a typeface without one, from a synthesized
sfnt of its tables and units per em, variations, the 16.16 scale and `HBLockedFaceCache`) and
`unicode_shaper` (`SkShaper_skunicode.cpp`). `SkShapers::BestAvailable` and `SkShaper::Make` use them
whenever a Unicode implementation exists. The 36 `ShaperTest` entries pass.

HarfRust is the vendored `third_party/harfrust` (`SKIA-RUST.md` lists the patches). shaper-diff found
two more classes than §2.5: `fallback-space-advance` (HarfBuzz divides the font's scale for the fallback
space widths) and HarfBuzz's own `roundf` (`floorf (x + .5f)`, `hb-algs.hh#L329-L337`), which the anchor
patch needed. Modes b and c now shape through `skia-rust-shaper` with the real `Font` of a Fontations
typeface and the real `TestTypeface`, and the C++ driver reads their numbers as tables: every row of both
modes is exact (5,184 and 72). Mode a (HarfRust's own font functions, which Skia's callbacks replace) keeps
108 rows: `builtin-extents` 82, `builtin-advance-rounding` 26.

Differences from the C++ that a caller can see: `BidiRunIterator` returns `None` instead of null (the
`Factory` method then returns one run at the given level); the buffer context (five code points on each
side, `hb_buffer_add_utf8`) is decoded with Rust's UTF-8 rules, which differ from HarfBuzz's only for
invalid UTF-8 in the context.

### Status of M0b and M8's font part (2026-10-12)

M0b is done except that the release asset is not yet published (the session that built it could not
create releases): `cargo xtask text-deps fetch`, `oracle/shaper-diff` with the three modes, the
expected files and the replay tests. M8's font part is in place: `skia_rust_tools::paragraph_fonts`
(`SKIA_RUST_PARAGRAPH_FONTS` = `--paragraph_fonts`, `release` selects the asset;
`SKIA_RUST_RUN_PARAGRAPH_TESTS_NEEDING_SYSTEM_FONTS=0` = `--norun_paragraph_tests_needing_system_fonts`;
`resource_font_paths` = the sorted listing of `ResourceFontCollection::getTypefaces`). Left for M8:
`ResourceFontCollection` and `TestFontCollection` themselves (they need text T19b's typeface and the
`TypefaceFontProvider` of M9), the registration of the fonts with a font manager, and the
`SKIP_IF_FONTS_NOT_FOUND` / `UNIX_ONLY_TEST` macros of the test harness, which belong with the first
paragraph test port (M11).

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
- These drivers are the seeds of M0b and M0c. M0b is `oracle/shaper-diff` (§2.5); M0c is `oracle/unicode-diff`
  (§3.4), which also compiles the ICU `common/*.cpp` with Skia's defines (`U_ENABLE_DYLOAD=0`,
  `U_USING_ICU_NAMESPACE=0`) and supplies an empty `icudt78_dat` instead of `source/stubdata`.

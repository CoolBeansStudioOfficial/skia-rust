# Design: codecs (Phase 4)

Status: proposed (2026-10-08). Pin: `chrome/m156` (`95ee33d7ec77`). Audience: every agent porting
Skia's `src/codec`, `src/encode`, `include/codec`, `include/encode`, the lazy-image path
(`SkImages::DeferredFromEncodedData`, `SkImageGenerator`) and the third-party decoders Skia wraps.

Skia references are `path#Lx-Ly` in the pinned tree (`third_party/skia`). Third-party sources are
named by their `DEPS` pin (§1.1); they are not in `third_party/skia` (no `third_party/externals`
in the shared checkout) and are fetched separately (task C0b). Manifest counts are from
`inventory/manifest.toml` at `9db0ed8`. Counts marked *file scan* come from grepping a test or GM
source for decode/encode calls and resource names, not from running it; read them as estimates.

## Decisions at a glance

1. **The oracle used libjpeg-turbo, libpng, libwebp and Wuffs, not Skia's Rust codecs.** The
   m156 oracle builds are `is_official_build=false` on Windows x64 with clang-cl, so
   `gn/skia.gni` defaults apply: `skia_use_libjpeg_turbo_{decode,encode}`,
   `skia_use_libpng_{decode,encode}`, `skia_use_libwebp_{decode,encode}`, `skia_use_wuffs`,
   `skia_use_jpeg_gainmaps` (dev build) and `skia_use_expat` are on; every `skia_use_rust_*`
   flag, AVIF, JPEG XL and NDK images are off; RAW is off on Windows (`skia_use_piex =
   !is_win`). BMP and WBMP are always on. ICO comes with libpng. GIF is Wuffs only (§1.1).
2. **x64 goldens came from the portable C paths of libjpeg-turbo.** Skia's
   `third_party/libjpeg-turbo/BUILD.gn` adds `WITH_SIMD` only for non-Windows arm/arm64 (NEON
   intrinsics), so the oracle ran `jidctint.c`, `jdsample.c`, `jdcolor.c` and friends.
   libwebp ran its runtime-dispatched SSE2/SSE4.1 kernels, libpng its SSE2 unfilters, zlib its
   SIMD adler/CRC/chunk-copy, and Wuffs its SSE4.2 swizzlers. All of those are integer code
   with one defined answer (§4). **We port the portable C paths only**, with no `unsafe`, and
   check them against the pinned C built on Linux (decision 9).
3. **Port the decoders, don't depend on look-alikes.** libjpeg-turbo 3.1.0, libpng 1.6.56,
   libwebp 1.4.0, Wuffs v0.3 (GIF + LZW + the base pieces Skia uses) and Chromium zlib
   1.3.0.1-motley are ported function by function, but only the code Skia calls. A Rust crate
   that "decodes JPEG" differs in IDCT, upsampling, rounding, and in what it does with truncated
   or corrupt input, and the tests check all of those. The rule (§2): **depend only where the
   dependency *is* Skia's implementation** (the `png` crate behind `SkPngRustCodec`, as skrifa
   is for text), or where every conforming implementation gives the same observable result and
   the tests can't tell them apart (XML parsing for XMP; Q4).
4. **zlib is ported, not a crate**, inflate first and deflate later. Inflate is small (~2.5k C
   lines), and libpng's progressive reader depends on zlib's streaming behaviour (how many
   bytes come out of a truncated stream, which error is reported), which incomplete-image tests
   observe. Chromium's deflate is **not** canonical zlib (it hashes with `value * 66521`,
   `contrib/optimizations/insert_string.h`, and forces `hash_bits >= 15`), so only a port
   reproduces Skia's PNG and PDF bytes. `zlib-rs`/`miniz_oxide` would not.
5. **Crates: one for Skia's codec layer, one per wrapped library.** `skia-rust-codec` holds the
   Codec API, Skia's own decoders (BMP, WBMP, ICO), the `Sk*Codec` glue, encoders' Skia side,
   EXIF/XMP/MPF and the image generator. The ports are separate crates with their own licences
   and no Skia dependency: `skia-rust-zlib`, `skia-rust-libpng`, `skia-rust-libjpeg`,
   `skia-rust-libwebp`, `skia-rust-wuffs` (§3). `GainmapInfo` and `HdrMetadata` (Skia's
   `include/private`) move into core, because core's gainmap shader uses them.
6. **The public API is skia-safe's**: `codec::Codec<'a>`, `codecs::Decoder`, `png_decoder`,
   `jpeg_decoder`, …, `png_encoder`/`jpeg_encoder`/`webp_encoder`,
   `images::deferred_from_encoded_data`. Inside, SkCodec's base/virtual split becomes a
   `CodecBase` struct plus a `CodecImpl` trait whose methods receive `&mut CodecBase` (§5).
   Two deviations go to `docs/API_MAPPING.md`: streams must be `Send` (so a codec can back a
   shared lazy image), and incremental decoding returns a guard that borrows the destination
   pixels instead of storing a raw pointer.
7. **Lazy images live in core, codec support in codec.** `ImageGenerator` (trait) and
   `ImageLazy` (`SkImage_Lazy`) go in core. `CodecImageGenerator`,
   `image_generators::make_from_encoded` and `images::deferred_from_encoded_data` go in
   `skia-rust-codec`, and the facade re-exports them at skia-safe's paths. Core never calls a
   codec, so the layering holds without a registry hook (§6).
8. **First wave: Codec API + BMP/WBMP + zlib + libpng + PNG/ICO + lazy images.** By file scan,
   that makes PNG decodable for about 125 non-codec GMs (41 files, mostly `mandrill_*.png` and
   `color_wheel.png`) and about 30 codec unit tests, and it brings in the shared machinery
   (swizzler, sampler, colour transform, frame holder) every later codec needs. Then GIF
   (Wuffs; small, unlocks 9 `GifTest` + ~15 more), JPEG (libjpeg-turbo + EXIF + YUV; the biggest
   GM lever after PNG), WebP, the encoders, and finally gainmaps and the HDR shader, which wait
   for SkSL's `RuntimeEffect` (§10).
9. **Without the Windows oracle, decoder-level differential testing is the debugging tool.**
   `oracle/codec-diff/` (task C0b), modelled on `oracle/skcms-diff`, builds the *pinned C* of
   zlib/libpng/libjpeg-turbo/libwebp/Wuffs on Linux with Skia's defines and
   `-ffp-contract=off`, then dumps decoded buffers, rows-decoded counts and error codes for every
   resource image and a set of truncations. `cargo test` replays the committed hashes. It needs
   no Skia build, it runs where agents run, and it shows the first differing row before any GM
   hash is involved. It can also build the SIMD paths, which answers the arm64 question (§4).
10. **`dm-image` (236 entries) has no goldens.** The oracle ran `--src gm` only (`hashes-m156.json`
    holds 2,727 `gm` results per tier and no `image` results). 19 entries are files the oracle
    could not decode (AVIF 9, DNG 3, `.exif` 4, JXL 1, DDS 1, KTX 1) and become `excluded`. The
    other 217 stay `todo` with reason "needs DM `--src image` goldens". C22 ports DM's image
    sources anyway, so they can be checked the day goldens exist (Q2).
11. **Out of scope, excluded with reasons** (C0): AVIF (13 unit), JPEG XL (1), NDK
    decode/encode (12 in codec, 8 core, 3 skcms), RAW (`Codec_raw`, `Codec_raw_notseekable`:
    both are compiled out without `SK_CODEC_DECODES_RAW`). That is 39 entries.
12. **Skia's Rust-backed codecs are a separate, optional wave (C-R; Q1).** 121 entries test
    `SkPngRustCodec` (53 + 3 encoder), `SkJpegRustCodec` (18), BMP/ICO via the `image` crate
    (16 + 18), and Skia's own Rust EXIF/ICC parsers (8 + 5 in skcms). They don't affect any
    golden. PNG (`png =0.18.1` + Skia's 345-line `captured-chunks.patch`) and JPEG (`zune-jpeg
    =0.5.16-rc1`, `jpeg-encoder =0.6.0`) are cheap once the Codec API exists. BMP/ICO need
    `image` at a git revision, which `cargo deny` (`unknown-git = "deny"`) and crates.io
    publishing both forbid. Recommendation: keep PNG/JPEG/EXIF/ICC in scope as C-R, and exclude
    BMP/ICO-Rust (34) until an `image` release contains that revision.

---

## 1. What the oracle built and what the tests compare

### 1.1 Codec configuration of the m156 oracle

The oracle's GN args (`oracle/tiers.toml`, echoed in every tier's `toolchain` in
`hashes-m156.json`) are only `is_debug=false is_official_build=false skia_enable_tools=true
skia_use_gl=false` plus `-ffp-contract=off`. Everything else is a default from `gn/skia.gni` on
`is_win`, `target_cpu = "x64"`, `is_clang` (clang-cl):

| Backend | GN flag (value in the oracle) | Defines | Library and `DEPS` pin | Skia glue |
|---|---|---|---|---|
| JPEG decode | `skia_use_libjpeg_turbo_decode` (true) | `SK_CODEC_DECODES_JPEG` | libjpeg-turbo **3.1.0** (Chromium `libjpeg_turbo@e14cbfaa`), `TURBO_FOR_WINDOWS`, `C_/D_ARITH_CODING_SUPPORTED`, `USE_CLZ_INTRINSIC`, **no `WITH_SIMD` on x64** | `SkJpegCodec`, `SkJpegDecoderMgr`, `SkJpegSourceMgr`, `SkJpegUtility`, `SkJpegMetadataDecoderImpl` |
| JPEG gainmaps | `skia_use_jpeg_gainmaps` (= `is_skia_dev_build`, true) | `SK_CODEC_DECODES_JPEG_GAINMAPS` | expat (`SK_XML`) | `SkJpegMultiPicture`, `SkJpegSegmentScan`, `SkJpegXmp`, `SkXmp`, `SkGainmapInfo`, `SkJpegGainmapEncoder` |
| JPEG encode | `skia_use_libjpeg_turbo_encode` (true) | `SK_CODEC_ENCODES_JPEG` | same | `SkJpegEncoderImpl`, `SkJPEGWriteUtility` |
| PNG decode | `skia_use_libpng_decode` (true); `skia_use_rust_png_decode` (false) | `SK_CODEC_DECODES_PNG`, `_WITH_LIBPNG` | libpng **1.6.56** (`skia/third_party/libpng@d5515b5b`), `PNG_SET_OPTION_SUPPORTED`, `PNG_INTEL_SSE` | `SkPngCodec`, `SkPngCodecBase`, `SkPngCompositeChunkReader` |
| PNG encode | `skia_use_libpng_encode` (true) | `SK_CODEC_ENCODES_PNG_WITH_LIBPNG` | same + zlib | `SkPngEncoderImpl`, `SkPngEncoderBase` |
| ICO | with libpng decode | `SK_CODEC_DECODES_ICO` | — | `SkIcoCodec` (embeds BMP and PNG codecs) |
| BMP, WBMP | always (`skia_public` config) | `SK_CODEC_DECODES_BMP`, `_WBMP` | — | `SkBmp*Codec`, `SkWbmpCodec` |
| GIF | `skia_use_wuffs` (true) | `SK_HAS_WUFFS_LIBRARY`, `SK_CODEC_DECODES_GIF` | Wuffs **v0.3** release C (`wuffs-mirror-release-c@e3f919cc`, `wuffs-v0.3.c`), modules BASE, GIF, LZW | `SkWuffsCodec` |
| WebP decode/encode | `skia_use_libwebp_{decode,encode}` (true) | `SK_CODEC_DECODES_WEBP`, `_ENCODES_WEBP` | libwebp **1.4.0** (`libwebp@845d5476`), `WEBP_SWAP_16BIT_CSP`, SSE2 + SSE4.1 (`-msse4.1` file set), runtime `VP8GetCPUInfo` | `SkWebpCodec`, `SkWebpEncoderImpl` |
| zlib | `skia_use_zlib` (true) | — | Chromium zlib **1.3.0.1-motley** (`zlib@646b7f56`); SSSE3 adler32, SSE4.2+PCLMUL crc32, SSE2 inflate chunk copy, ANZAC hash in deflate | (libpng, PDF) |
| XML | `skia_use_expat` (true) | `SK_XML` | expat (`libexpat@6154446f`) | `SkXMLParser`, `SkDOM` |
| colour | always | — | skcms (in tree). **skcms most likely ran its SKX kernels on every x64 tier**: its own CPUID check ignores `SKIA_ORACLE_CPU_CAP`, and HSW/SKX are compiled in for clang on x64 (`modules/skcms/BUILD.gn`, `src/skcms_internals.h#L110-L128`). This assumes clang-cl defines `__x86_64__`, which we believe but haven't verified (R4) | `SkCodec::applyColorXform`, `SkCodecColorProfile` |
| AVIF, JPEG XL | off | — | — | excluded |
| RAW (DNG) | `skia_use_dng_sdk` is true, but `raw` also needs `skia_use_piex = !is_win` (false) | — | — | excluded |
| NDK images | `is_android` only | — | — | excluded |
| Rust PNG/BMP/ICO/JPEG/EXIF/ICC | `skia_use_rust_*` (all false) | — | `png 0.18.1`, `image@5d0418d0`, `zune-jpeg 0.5.16-rc1`, `jpeg-encoder 0.6.0` (`MODULE.bazel`) | wave C-R (Q1) |

How Skia drives each library matters as much as which one it uses:

- **libjpeg-turbo:** defaults throughout: `dct_method = JDCT_ISLOW`, `do_fancy_upsampling = TRUE`,
  `do_block_smoothing = TRUE`. Merged upsampling (`jdmerge.c`) is therefore never used
  (`use_merged_upsample` returns false when fancy upsampling is on, `jdmaster.c#L35-L60` at the
  pin). Fancy upsampling itself is skipped when `min_DCT_scaled_size == 1`, i.e. at scale 1/8
  (`jdsample.c`). Output spaces are `JCS_EXT_RGBA`/`JCS_EXT_BGRA`/`JCS_GRAYSCALE`/`JCS_CMYK`, and
  `JCS_RGB565` with `dither_mode = JDITHER_NONE` (`SkJpegCodec.cpp#L300-L356`). Native scaling
  uses `scale_num/scale_denom` (1/8 … 8/8), so the reduced IDCTs (`jidctred.c`) and the
  `jpeg_idct_NxN` family in `jidctint.c` are needed. Other entry points: `jpeg_crop_scanline`
  and `jpeg_skip_scanlines` for subsets, `raw_data_out` + `jpeg_read_raw_data` for YUV planes,
  `buffered_image` for progressive input, and `jpeg_save_markers` for APP1 (EXIF/XMP), APP2
  (ICC, MPF).
- **libpng:** progressive reading only (`png_process_data` with Skia's own chunk walk,
  `SkPngCodec.cpp#L150-L260`), `PNG_MAXIMUM_INFLATE_WINDOW`, unknown chunks always kept for
  the chunk reader, and only the transforms `png_set_strip_16`, `png_set_packing`,
  `png_set_tRNS_to_alpha`, `png_set_expand_gray_1_2_4_to_8` and interlace handling
  (`SkPngCodec.cpp#L778-L970`). Gamma correction stays off. Everything else (swizzling,
  premultiplication, colour transforms) is Skia's own `SkSwizzler`/skcms.
- **libwebp:** `WebPDemux`, `WebPIDecode`/`WebPIUpdate` (incremental), output `MODE_rgbA`/`bgrA`
  (premultiplied by libwebp), `MODE_RGBA`/`BGRA`, `MODE_RGB_565` (BGR order under
  `WEBP_SWAP_16BIT_CSP`), `use_cropping`, `use_scaling` (the `WebPRescaler`). Animated frames
  are blended by Skia through `SkRasterPipeline` (`SkWebpCodec.cpp#L520-L535`), so those
  pixels depend on the CPU tier like any other pipeline output.
- **Wuffs:** `wuffs_gif__decoder` with `QUIRK_IGNORE_TOO_MUCH_PIXEL_DATA`, decoding into
  `BGRA_NONPREMUL`, `RGBA_NONPREMUL` or `BGR_565` with `SRC`/`SRC_OVER` blending. The
  "two-pass" path decodes into a private buffer and composites with `SkCanvas::drawImage`
  (`SkWuffsCodec.cpp#L656-L800`), which is also tier dependent.

### 1.2 What each kind of test compares

| Kind | Count (codec module) | What passes it | Oracle needed? |
|---|---|---|---|
| `unit` | 288 (1 excluded) | Self-checking C++ assertions: decode results, MD5 equality between decode modes, rows decoded, round trips with tolerances written into the test | No |
| `gm` | 49 here, plus GMs in every other module that draw decoded resources | Golden hashes in `goldens-m156` (about 32 of the 49 have CPU goldens; the rest are GPU-only YUV/texture GMs that skip on raster) | Already published |
| `dm-image` | 236 | DM `--src image`: every `CodecSrc`/`AndroidCodecSrc`/`ImageGenSrc`/`BRDSrc` mode × colour type × alpha × scale (`dm/DM.cpp#L727-L900`) | **Yes; not published** |
| `bench` | 35 | Perf gate (PLAN §9.2) | Server |
| `fuzz` | 5 | No crash | No |

Encoded bytes are compared byte for byte nowhere in the codec tests. `EncodeTest` checks
determinism (`data0->equals(data1)` for identical inputs), relative sizes between options and
round-trip pixels within the tolerances the test states. GMs that encode (`encode*`,
`jpg_color_cube`, `image.cpp`) draw the *decoded* result. Lossless round trips (PNG, WebP
lossless) therefore depend only on the encoder's pixel conversion. Lossy round trips (JPEG,
WebP lossy) depend on every arithmetic decision in the encoder, so the encoders must be ported
exactly as well (§7).

### 1.3 Manifest inventory (codec module, 613 entries)

| File | Entries | Wave (§10) | Note |
|---|---|---|---|
| `tests/CodecTest.cpp` | 57 | A ≈30, B ≈8, C ≈10, D 3, E 2, excluded 2 | file scan |
| `tests/SkPngRustDecoderTest.cpp` | 53 | C-R | `skia_use_rust_png_decode` |
| `tests/SkIcoRustDecoderTest.cpp`, `SkBmpRustDecoderTest.cpp` | 18 + 16 | C-R (excluded for now) | `image` git rev |
| `tests/SkJpegRustCodecTest.cpp` | 18 | C-R | zune-jpeg |
| `tests/AvifTest.cpp`, `JpegxlTest.cpp` | 13 + 1 | excluded | |
| `tests/EncodeTest.cpp` | 11 | E | |
| `tests/NdkEncodeTest.cpp`, `NdkDecodeTest.cpp` | 9 + 3 | excluded | Android only |
| `tests/GifTest.cpp` | 9 | B | |
| `tests/ExifTest.cpp` | 9 | C (7), D (1), E (1) | `SkExif` parse/write needs no decoder |
| `tests/JpegGainmapTest.cpp` | 9 | C (segment scan, MPF, XMP parse: 6), E (3 encode) | |
| `tests/RustExifTest.cpp` | 8 | C-R | Skia's own Rust |
| `tests/CodecPartialTest.cpp` | 8 | A 3, B 4, D 1 | |
| `tests/CodecAnimTest.cpp` | 7 | B/D | `AnimCodecPlayer` needs Skottie's player: last |
| `tests/AndroidCodecTest.cpp` | 5 | A 4 (wide/P3/HLG/PQ PNGs), D 1 | |
| `tests/ImageTest.cpp` | 5 (1 excluded) | A/E | |
| `tests/GainmapShaderTest.cpp` | 4 | G | runtime effect |
| `tests/BadIcoTest.cpp`, `SkJpegXmpTest.cpp`, `PngGainmapTest.cpp`, `YUVTest.cpp`, `SkPngRustEncoderTest.cpp` | 3 each | A, C, A/E, C/E, C-R | |
| `tests/UnicodeTest.cpp` (2), `TypefaceTest.cpp::TypefaceGlyphToUnicode` (1) | 3 | — | misfiled: move to `text` (C0) |
| 1 each: `CodecExactReadTest`, `CodecRecommendedTypeTest`, `EncodedInfoTest`, `IndexedPngOverflowTest`, `InvalidIndexedPngTest`, `WebpTest`, `YUVCacheTest` | 7 | A, E, E, A, A, D, C | |
| `gm/*` | 49 | A–E | §10 per GM file |
| `dm/image/*` | 236 | 217 `todo` (no goldens), 19 excluded | §9.4 |
| `bench/DecodeBench`, `EncodeBench`, `WebpBlendBench` | 6 + 28 + 1 | perf, after E | |
| `fuzz/FuzzEncoders.cpp` | 5 | E (PNGRustEncoder: C-R) | |

Codec work in other modules: `core` `ImageTest` (20, of which about 6 decode or encode),
`AnimatedImageTest` (6), `ImageGeneratorTest` (2), `ImageGeneratorOrientationTest` (1),
`SkXmpTest` (7), `HdrMetadataTest` (9 core + 2 effects), `MipMapTest` (2), `Skbug6389`,
`SamplingTest`, `ImageIsOpaqueTest`, `SerialProcsTest`, `SerializationTest` (1 each);
`pdf` `PDFJpegEmbedTest` (2). By file scan, **about 200 `todo` GMs outside the codec module
live in files that decode a resource or encode**: about 125 need only PNG, 24 PNG + JPEG, 17
JPEG (9 of them also encode), 5 GIF, 2 WebP, 7 only encode, 3 all formats, and 17 load
resources by computed names. Many of them also wait on other work (image filters, runtime
effects, text). So these are ceilings, not promises.

---

## 2. Port or depend: the rule

PORTING §8 says "no dependency may replace ported Skia behaviour". For the libraries Skia
wraps, apply it like this:

| Library | Decision | Why |
|---|---|---|
| libjpeg-turbo (decompress + compress, 8-bit) | **Port** | IDCT, upsampling, colour conversion, scaled IDCTs, block smoothing and skip/crop behaviour all shape the pixels. Rust JPEG crates (zune-jpeg, jpeg-decoder) use other IDCTs and upsamplers. |
| libpng (read path, then write path) | **Port** | Pixel bytes are fixed by the PNG spec, but the tests check libpng's *behaviour*: when progressive rows are delivered, which chunks it accepts or rejects (CRC, `IDAT` order, oversized `PLTE`, `tRNS` edge cases), what `png_get_*` reports, and when a truncated stream stops. `CodecPartialTest`, `IndexedPngOverflowTest`, `InvalidIndexedPngTest` and `Codec_InvalidImages` depend on that. |
| zlib (Chromium) | **Port** | See decision 4. |
| libwebp (decoder, demux; encoder, mux later) | **Port** | The VP8 transforms and loop filter are fixed by the spec, but YUV→RGB, fancy upsampling, premultiplication, the rescaler, incremental-decode row counts, and the whole lossy encoder are libwebp's choices. |
| Wuffs (GIF, LZW, base swizzler/blend) | **Port** (transcribe the generated C) | Wuffs' suspension points decide how many rows a truncated GIF yields (`Codec_GifTruncated*`, `Codec_partialWuffs`). The SRC_OVER blend arithmetic is Wuffs'. |
| skcms | already ported | `skia-rust-skcms` |
| expat | **Depend** on a small, safe XML tokenizer behind a faithful port of `SkXMLParser`/`SkDOM` (Q4) | XMP and SVG see only element/attribute/text events. A conforming parser gives the same events for well-formed input, and the only malformed-input test (`SkXmp_invalidXml`) needs plain rejection of truncated XML. Expat is about 8k lines for no observable gain. |
| `png`, `zune-jpeg`, `jpeg-encoder` (C-R only) | **Depend**, `=`-pinned to Skia's versions | They *are* the implementation behind `SkPngRustCodec`/`SkJpegRustCodec`, as skrifa is for Fontations (`docs/design/text.md` §2.2). |

Port only the paths Skia reaches. In libjpeg-turbo that means the 8-bit decompressor and
compressor. The 12/16-bit builds (`libjpeg12`/`libjpeg16` in GN) exist only so the library
links, because Skia always calls the 8-bit API. Leave out `jidctflt`, `jidctfst`, `jquant1/2`,
`jdmerge` (see §1.1), the transcoder (`jdtrans`/`jctrans`) and TurboJPEG. Lossless JPEG
(`jdlossls`, `jddiffct`, `jdlhuff`) stays out unless a resource needs it. In libpng, leave out
the simplified API (`png_image_*`, most of `pngread.c`), `png_read_png`, and every transform
Skia doesn't set. In libwebp, leave out the MIPS/MSA/NEON/SSE files (§4) and the `sharpyuv`
encoder path unless `use_sharp_yuv` is reached. In Wuffs, port only `base` (io buffers, status,
pixel config/buffer/swizzler for the formats in §1.1), `lzw` and `gif`.

Each port keeps the library's own structure (file → module, function → function, comments
naming the C function) and its licence header. `// Port of:` links use the library and pin,
e.g. `// Port of: libjpeg-turbo src/jidctint.c#L172-L320 (libjpeg_turbo@e14cbfaa)`.

---

## 3. Crates and layering

```
skia-rust-zlib      (std only)                              Zlib licence
skia-rust-libpng    → zlib                                  libpng-2.0
skia-rust-libjpeg   (std only)                              IJG AND BSD-3-Clause AND Zlib
skia-rust-libwebp   (std only)                              BSD-3-Clause
skia-rust-wuffs     (std only)                              Apache-2.0
skia-rust-codec     → core, raster, skcms, simd, the five above
skia-rust (facade)  → codec behind feature "codec" (default, PLAN §3.2)
```

- **Why separate crates.** Each library has its own licence (`deny.toml` must allow `IJG` and
  `libpng-2.0`, added in C0). Each is 3–15k lines, so splitting gives parallel compilation and
  lets agents work in parallel without touching the same `lib.rs`. None of them knows about
  Skia, so their tests (§9.1) are pure library tests. They sit beside `simd` at the bottom of
  the layering and never depend on core. If one needs libm (the libwebp encoder calls `pow`),
  it uses the UCRT-exact `libm` module (PR #87), which should then move to `skia-rust-base`
  (`docs/design/sksl.md` §2.2) so these crates can reach it.
- **`skia-rust-codec` layout** (PORTING §2 naming): `codec` (SkCodec), `codec_priv`,
  `encoded_info`, `swizzler`, `mask_swizzler`, `sampler`, `sampled_codec`, `android_codec`,
  `android_codec_adapter`, `frame_holder`, `color_palette`, `codec_color_profile`,
  `codec_image_generator`, `image_generator_from_encoded`, `bmp_codec` (+ `bmp_standard_codec`,
  `bmp_mask_codec`, `bmp_rle_codec`, `bmp_base_codec`), `wbmp_codec`, `ico_codec`, `png_codec`,
  `png_codec_base`, `png_composite_chunk_reader`, `jpeg_codec`, `jpeg_decoder_mgr`,
  `jpeg_source_mgr`, `jpeg_utility`, `jpeg_metadata_decoder_impl`, `jpeg_multi_picture`,
  `jpeg_segment_scan`, `jpeg_xmp`, `xmp`, `exif`, `tiff_utility`, `wuffs_codec`, `webp_codec`,
  `encode::{encoder, png_encoder_impl, png_encoder_base, jpeg_encoder_impl, jpeg_write_utility,
  jpeg_gainmap_encoder, webp_encoder_impl, icc}`, `decoders::{png, jpeg, gif, webp, bmp, ico,
  wbmp}` (the `SkXxxDecoder` namespaces), and `android::animated_image` (`SkAnimatedImage`,
  `src/android`, enabled because `skia_enable_android_utils` is on in dev builds).
- **Core additions**: `image_generator` (`SkImageGenerator`), `image_lazy` (`SkImage_Lazy`),
  `gainmap_info` (`SkGainmapInfo`, header in `include/private`, `.cpp` in `src/codec` but
  dependency-free), `hdr_metadata` + `hdr_agtm` (`SkHdrMetadata`, `SkHdrAgtm*`), `md5`
  (`SkMD5`, used by codec tests and DM), `yuva_info`/`yuva_pixmaps` if not already present,
  `xml::{parser, dom}` (`SkXMLParser`, `SkDOM`; shared with SVG later). The gainmap shader
  (`SkGainmapShader`, `src/shaders`) goes into effects next to the other runtime-effect shaders.
- **`skia-rust-simd` additions**: the remaining `SkOpts` swizzles the swizzler uses
  (`RGB_to_RGB1`, `RGB_to_BGR1`, `gray_to_RGB1`, `grayA_to_RGBA`, `grayA_to_rgbA`,
  `inverted_CMYK_to_RGB1`, `inverted_CMYK_to_BGR1`, `memset16/32/64`), portable kernels first,
  beside the five that `swizzle.rs` already has.

---

## 4. SIMD

**What the goldens embed.** For x64 (the only goldens so far): libjpeg-turbo C paths; libwebp
SSE2/SSE4.1 (CPUID-dispatched inside libwebp, so the same on every Skia tier); libpng SSE2
unfilter; zlib SIMD checksums and chunk copies; Wuffs SSE4.2 swizzlers; Skia's own `SkOpts`
swizzles at the tier's level; skcms SKX. Every one of these except skcms is integer code
computing a defined function: PNG unfiltering and adler32/CRC32 are defined bit for bit, Wuffs
generates its SIMD from the same source, and libwebp's `yuv_sse2.c`/`upsampling_sse2.c`/
`alpha_processing_sse2.c` use the C code's fixed-point constants (`MultHi` with 14-bit factors).
So **the portable ports reproduce the x64 goldens by construction**. C0b's `codec-diff` checks
that claim against the real SSE builds on Linux once (R3) rather than taking it on trust.

**arm64.** Skia's arm64 builds (macOS/Linux, the future `arm64-neon` tier) enable libjpeg-turbo
NEON (`jidctint-neon.c`, `jdsample-neon.c`, `jdcolor-neon.c`, …), libwebp NEON, libpng NEON and
zlib NEON. Upstream libjpeg-turbo's regression suite expects the same MD5 with and without SIMD
for the integer DCT, but we have not verified that per function for every
upsampling/colour-conversion variant Skia uses (R2). Until an arm64 oracle exists, `codec-diff`
runs on the Linux aarch64 CI runner and compares the NEON build of the pinned C with our ports.
That is the authoritative check for arm64, and any NEON-only behaviour it finds becomes a
`Tier::Neon` variant in the port.

**Rules for adding SIMD later (perf).**
- Decoder kernels go into `skia-rust-simd` under `codec::{idct, upsample, color_convert,
  png_unfilter, webp_dsp, …}`, each with a scalar twin. The twin is the function in the port
  crate, re-exported. Proptest plus `codec-diff` check equality. Everything in
  `docs/UNSAFE.md` applies.
- Unlike raster pipeline kernels, a decoder kernel's result **must not** depend on the tier.
  Dispatch picks the fastest kernel the host has (not `oracle_selection`), and the tests force
  every tier and assert identical bytes.
- No decoder SIMD until the scalar port matches everything (PLAN §11). The codec perf benches
  (`DecodeBench`, 6) come after the waves in §10.
- skcms SKX vs our scalar skcms is a different question: §11 R4.

---

## 5. The Codec API

### 5.1 Public shape (skia-safe)

`skia_rust::codec::{Codec, Options, FrameInfo, Result, SelectionPolicy, ZeroInitialized,
ScanlineOrder, IsAnimated, EncodedImageFormat, EncodedOrigin, codec_animation, pixmap_utils}`,
`skia_rust::codecs::{Decoder, deferred_image}`, the per-format modules `bmp_decoder`,
`gif_decoder`, `ico_decoder`, `jpeg_decoder`, `png_decoder`, `webp_decoder`, `wbmp_decoder`
(and `png_rust_decoder` with C-R), `png_encoder`, `jpeg_encoder`, `webp_encoder` (each `Options`
plus `encode`, `encode_pixmap`, `encode_image`), and `images::deferred_from_encoded_data`. Names,
argument order and return types follow `third_party/rust-skia/skia-safe/src/codec/*` and
`encode_/*`. Skia-only API that skia-safe lacks (`SkAndroidCodec`, `getGainmapInfo`,
`getHdrMetadata`, `SkCodecs::Register`, `SkPngChunkReader`) follows the mechanical rule with
`#[doc(alias)]`.

### 5.2 Base and implementations

SkCodec is a base class with heavy non-virtual logic (`getPixels` → `handleFrameIndex` →
`rewindIfNeeded` → `onGetPixels`; scanline and incremental state; colour-transform setup), and
subclasses call back into it (`this->stream()`, `this->colorXform()`,
`this->applyColorXform()`, `this->dstInfo()`). Port that as:

```rust
pub struct Codec<'a> {
    base: CodecBase<'a>,                 // SkCodec's fields + non-virtual methods
    imp: Box<dyn CodecImpl + Send + 'a>, // SkPngCodec, SkJpegCodec, …: the subclass fields
}

pub(crate) struct CodecBase<'a> {
    encoded_info: EncodedInfo, src_xform_format: XformFormat, origin: EncodedOrigin,
    stream: Option<Box<dyn Stream + Send + 'a>>, needs_rewind: bool,
    dst_info: ImageInfo, options: Options, xform: Option<ColorXform>, /* … */
    curr_scanline: i32, started_incremental_decode: bool, /* … */
}

pub(crate) trait CodecImpl {
    fn on_get_pixels(&mut self, base: &mut CodecBase<'_>, info: &ImageInfo,
                     dst: &mut [u8], row_bytes: usize, opts: &Options,
                     rows_decoded: &mut i32) -> Result;
    fn on_rewind(&mut self, _base: &mut CodecBase<'_>) -> bool { true }
    fn on_get_scaled_dimensions(&self, base: &CodecBase<'_>, scale: f32) -> ISize { … }
    // … one method per SkCodec virtual, with SkCodec's default body as the default
}
```

- Every `CodecImpl` method gets `&mut CodecBase` explicitly. `this->foo()` in a subclass becomes
  `base.foo()`. No back-pointers and no `RefCell`.
- Codecs that wrap codecs (`SkIcoCodec`'s embedded PNG/BMP codecs, `SkSampledCodec` /
  `SkAndroidCodecAdapter` around an `SkCodec`, gainmap codecs) own `Codec` values. Skia uses
  `unique_ptr` there, and so do we.
- The pixel destination is `&mut [u8]` + `row_bytes`. Fills (`SkSampler::Fill`) and swizzles
  index the slice with Skia's offsets.
- **Incremental decoding** keeps the destination between calls. skia-safe's
  `start_incremental_decode(&mut self, …, dst: &mut [u8], …) -> Result` hides a stored pointer.
  Here it returns `Result<IncrementalDecode<'_, '_>, Result>`, a guard that borrows both the
  codec and `dst` and has `incremental_decode(&mut self) -> (Result, Option<usize>)`. Ported
  tests change call sites mechanically. Record this in API_MAPPING.
- **Streams.** `Codec` owns a `Box<dyn Stream + Send + 'a>` (core's `Stream`, with Skia's
  optional `rewind`/`seek`/`peek`/`get_memory_base`/`get_length`). skia-safe's
  `from_stream<T: io::Read + io::Seek>` wraps the reader in an adapter that implements
  `Stream` (and `peek` via `SkFrontBufferedStream`, already in core). `from_data(Data)` uses
  `MemoryStream`. The `Send` bound lets a codec back a shared `Image` (§6). Every core stream
  is `Send`.
- **Decoder registry.** `SkCodecs::Decoder { id, is_format, make_from_stream }` is a plain value.
  The default list holds the compiled-in decoders in `SkCodec.cpp#L89-L136` order (PNG, JPEG,
  WebP, GIF, ICO, BMP, WBMP). `SkCodecs::Register` is public Skia API, and 4 tests
  (`DEF_SERIAL_TEST` in `CodecTest`, `BadIcoTest`) rely on its process-wide effect (ICO asks the
  registry for its PNG decoder). So it is a `LazyLock<RwLock<Vec<Decoder>>>`, documented as
  sanctioned process-wide state like `StrikeCache::global()` (`docs/design/text.md` §5.2). Serial
  tests take a shared test mutex and restore the list with a scoped guard, as
  `ScopedCodecDecoders` does.
- **Results** map one to one: `SkCodec::Result` is `codec::Result` (skia-safe re-exports the
  enum with the same variants). Functions that return `nullptr` + `Result*` return
  `Result<Codec, codec::Result>`, as skia-safe's `from_stream` does.

### 5.3 Shared machinery

- `SkSwizzler` (1,259 lines) and `SkMaskSwizzler` (575): a `RowProc` is a `fn` pointer chosen by
  the same `switch` in `CreateSwizzler`. `SkOpts::*` calls go through `skia-rust-simd`, which
  respects the tier (the results are the same on every tier, §4). Sampling (`fSampleX`,
  `fSrcOffset`, `fSwizzleWidth`) keeps Skia's integer arithmetic.
- Colour: `SkCodec::initializeColorXform`/`applyColorXform` call `skia_rust_skcms::transform`
  with Skia's `skcms_PixelFormat` and alpha choices. ICC parsing goes through
  `SkCodecColorProfile` → `skcms::parse`.
- `SkFrameHolder`/`SkFrame` (animation bookkeeping, required-frame computation) are pure logic,
  shared by Wuffs and WebP.
- `SkEncodedInfo`, `SkColorPalette`, `SkCodecPriv` (`get_scaled_dimension`, `is_coord_necessary`,
  `get_dst_coord`, …) are small and come first.

---

## 6. Lazy images and generators

- **Core:** `ImageGenerator` is a trait mirroring `SkImageGenerator`'s virtuals (`info`,
  `on_get_pixels`, `on_ref_encoded_data`, `on_query_yuva_info`, `on_get_yuva_planes`,
  `on_is_valid`, `on_is_protected`, `unique_id`) with `Send` as a supertrait. `ImageLazy`
  implements `ImageBase` with `ImageType::Lazy` (the variant exists already). It holds
  `SharedGenerator` = `Arc<Mutex<Box<dyn ImageGenerator>>>` (Skia: `SkImage_Lazy::SharedGenerator`
  with an `SkMutex`). `get_ro_pixels` decodes to `image_info()` exactly as
  `SkImage_Lazy::getROPixels` does, and `on_read_pixels`, `on_make_subset`,
  `on_make_color_type_and_color_space` and `on_ref_encoded` follow the C++. A
  `Picture`-backed generator (`SkPictureImageGenerator`) fits the same trait later.
- **Decoded-bitmap cache.** Skia keeps the decoded bitmap in the global `SkResourceCache`
  (`SkBitmapCache`), keyed by unique ID and honouring `CachingHint`. We keep it per image
  (`Mutex<Option<Bitmap>>`), filled only under `CachingHint::Allow`. This is the decision
  `docs/design/images.md` made for mipmaps. Tests that inspect the global cache
  (`ImageCacheTest`, `YUVCacheTest`) get a note and wait for a resource-cache port (R6).
- **Codec:** `CodecImageGenerator` (`SkCodecImageGenerator`: decodes with the stored
  `Codec` + `SkAndroidCodec` for scaled decodes, applies `EncodedOrigin` through
  `SkPixmapUtils::Orient`), `image_generators::make_from_encoded` (`SkImageGenerator_FromEncoded`)
  and `images::deferred_from_encoded_data` / `codecs::deferred_image`. The facade re-exports them
  under `images::`. That's the only way core and codec meet, so nothing in core needs a registry
  or a callback.
- **Picture serialization** of images (`SkReadBuffer::readImage`, `SkSerialProcs`) uses the
  codec only through procs the caller passes (m156). Tests in core that need it
  (`SerialProcsTest`, `SerializationTest`) live in the tests crate, which depends on everything.

---

## 7. Encoders

- **PNG** (`SkPngEncoderImpl`/`Base`, `SkEncoder`): libpng write path (`pngwrite.c`,
  `pngwutil.c`, `pngwtran.c` for `png_set_filler`/`png_set_swap`, `pngset.c`) + zlib deflate.
  Skia's pixel conversion (`SkPngEncoderBase` choosing 8/16-bit, unpremultiplying, colour
  conversion) decides the round-trip pixels. With a faithful deflate the bytes match Skia's too,
  which PDF needs later.
- **JPEG** (`SkJpegEncoderImpl`, `SkJPEGWriteUtility`, `SkJpegGainmapEncoder`): the libjpeg-turbo
  compressor with `jpeg_set_defaults` (ISLOW forward DCT `jfdctint.c`, Huffman with standard
  tables unless `optimize_coding`, 4:2:0/4:2:2/4:4:4 as Skia sets the sampling factors,
  `SkJpegEncoderImpl.cpp#L160-L195`), `jccolor`, `jcsample` (and its smoothing), `jcdctmgr`
  quantisation, markers (APP1/APP2/MPF). The YUV entry point uses `raw_data_in`.
- **WebP** (`SkWebpEncoderImpl`): libwebp's encoder, lossy (`method` 3 or 0 as Skia picks,
  `WebPConfigPreset(DEFAULT, quality)`) and lossless, `WebPMux` for ICC, `WebPAnimEncoder` for
  animations. This is the largest single port (about 18k C lines with mux and anim). It comes
  last, and its GMs (`encode-*-webp*`, 6 results) and tests (5) are its only consumers. Lossy
  output goes through float paths (`QualityToCompression` calls `pow`) and libm, so it must use
  the UCRT libm. The lossless encoder's float entropy estimates have SSE2 variants whose
  summation order may differ from C (R7). That doesn't matter for lossless *pixels*, but it does
  for bytes and sizes.

---

## 8. Metadata: EXIF, XMP, MPF, gainmaps, HDR

All of this is Skia's own code, ported directly:
- `SkExif` + `SkTiffUtility` (EXIF parse/write, orientation, HDR headroom): `ExifTest`, used by
  JPEG, PNG (`eXIf`) and WebP (`EXIF` chunk).
- `SkXmp` + `SkJpegXmp` (standard + extended XMP reassembly) over `SkDOM`/`SkXMLParser`:
  `SkXmpTest` (7), `SkJpegXmpTest` (3).
- `SkJpegSegmentScan`, `SkJpegMultiPicture` (MPF), `SkJpegMetadataDecoderImpl`, `SkJpegSourceMgr`
  (buffered/unbuffered segment access): `JpegGainmapTest` parse tests.
- `SkGainmapInfo` (ISO 21496-1 + Adobe hdrgm parse/serialize) and `SkHdrMetadata`/`SkHdrAgtm`
  (CLLI/MDCV, AGTM curves) in core (§3). Their math uses `std::log2`/`exp2`/`pow`, so it goes
  through the UCRT libm.
- The gainmap and AGTM **shaders** are runtime effects. `GainmapShaderTest` (4) and the
  `HdrMetadata_*Shader*` tests wait for SkSL's `RuntimeEffect` (`docs/design/sksl.md` S18).

---

## 9. Test harness

### 9.1 Library tests (no Skia involved)

`oracle/codec-diff/` (C0b):
- `cargo xtask codec-deps fetch` clones zlib, libpng, libjpeg-turbo, libwebp and wuffs at the
  `DEPS` revisions into `third_party/codec-externals/` (git-ignored, cached in CI like the Skia
  checkout).
- A C harness per library, built with clang on Linux (`-O2 -ffp-contract=off`, Skia's defines
  from §1.1, SIMD off; a second build with Skia's SIMD set for R2/R3), drives exactly the calls
  Skia makes: for example libjpeg `jpeg_start_decompress` → `read_scanlines` with every output
  space, scale and crop Skia uses; libpng `png_process_data` in 1-, 7- and 4096-byte chunks;
  Wuffs `decode_frame` with short reads. It prints one line per step: decoded-buffer SHA-256,
  rows produced, return/error code.
- Inputs: every file in `resources/images`, `resources/invalid_images`, plus deterministic
  truncations of each (every 1/16 of the length) and the corpus `codec-diff` grows from fuzz
  crashes.
- The Rust side prints the same text from the port crates. The expected text is committed per
  library (`expected/<lib>.txt`), so `cargo test -p skia-rust-libjpeg` checks it on every
  platform without C. Regenerate after editing cases. Same workflow as `oracle/skcms-diff`.

### 9.2 Unit tests

1:1 ports into `tests/src/unit/<file>.rs` as usual. Shared helpers come with the first test
that needs them: `SkMD5`, `ToolUtils::DecodeDataToBitmap{,WithColorType}` (tools
`DecodeUtils.cpp`), `GetResourceAsStream`/`GetResourceAsImage` in `tests/src/resources.rs`,
`ScopedCodecDecoders`, and `AnimCodecPlayer` (later; it is in `tools/`). Tests whose bodies are
`#if`'d out for the oracle's configuration (RAW, AVIF, Rust codecs) are excluded, not ported as
empty passes.

### 9.3 Resources in CI

The `test` jobs (4 platforms) don't fetch Skia today, so `resources/` is missing there and
every resource test skips (`skip_missing_resource!`). C0 adds the cached Skia checkout to the
test matrix (`actions/cache` keyed on `skia-pin.toml`, as the `inventory` job does), or a sparse
`resources/`-only fetch (52 MB). Without that, codec tests pass locally and silently skip on
CI, and the manifest's "every passing entry actually ran" check cannot hold.

### 9.4 GMs and DM image sources

- GMs: the existing harness (PORTING §11). GMs call `GetResourceAsImage` (lazy image) or
  `DecodeResourceAsBitmap`. In the oracle the resources were present (`--resourcePath
  resources`, `xtask/src/oracle.rs`), and the goldens show it: the codec GMs have distinct,
  non-blank hashes.
- `dm-image`: C22 ports `CodecSrc`, `AndroidCodecSrc`, `ImageGenSrc` and `BRDSrc` from
  `dm/DMSrcSink.cpp` and the mode enumeration in `dm/DM.cpp#L727-L900` into the GM harness as a
  second source kind with DM's result ids (`<config>/image/<options>/<name>`). An entry passes
  when all of its results match. Until goldens exist, the runner reports "no golden" and leaves
  the entry `todo`.

---

## 10. Work breakdown

Sizes: S < 1k Rust lines, M 1–3k, L 3–8k, XL > 8k. **(O)** = start on Opus (design-heavy),
**(S)** = start on Sonnet; everything else starts on Haiku (PLAN §8.3). "Unlocks" counts entries
that can flip if nothing else blocks them. Ranges and *file scan* figures are estimates.

### Wave C-0: bookkeeping and tools (parallel)

| ID | Task | Sources | Depends | Size | Unlocks |
|---|---|---|---|---|---|
| C0 | Manifest: exclude AVIF 13, JXL 1, NDK 12 (+8 core, +3 skcms), RAW 2, 19 undecodable `dm-image`; reason "needs DM --src image goldens" on the other 217 `dm-image`; move `UnicodeTest` (2) and `TypefaceGlyphToUnicode` to `text`; mark C-R entries per Q1. `deny.toml`: allow `IJG`, `libpng-2.0`. CI: Skia resources in the `test` matrix (§9.3) | manifest, CI | — | S | honest denominators |
| C0b **(S)** | `codec-deps fetch` + `oracle/codec-diff` skeleton (zlib harness first), CI caching, expected-file workflow | §9.1 | — | M | the debugging tool for every port |
| C1 **(O)** | Crate `skia-rust-codec`: `Codec`/`CodecBase`/`CodecImpl`, `Options`, `FrameInfo`, `Result`, registry, `make_from_stream` sniffing, scanline + incremental state machines, `EncodedInfo`, `codec_priv`, `ColorPalette`, colour-transform setup over skcms, `FrameHolder`, `EncodedOrigin`/`pixmap_utils`; core `md5`; simd swizzles (§3) | `SkCodec.cpp`, `SkEncodedInfo.cpp`, `SkCodecPriv.h`, `SkColorPalette.cpp`, `SkFrameHolder.h`, `SkCodecColorProfile.cpp`, `SkPixmapUtils`, `SkMD5.cpp`, `SkSwizzler_opts.inc` | — | L | `EncodedOriginToMatrixTest` 1 |

### Wave C-A: PNG, BMP, WBMP, ICO, lazy images → **first GM wave**

| ID | Task | Sources | Depends | Size | Unlocks |
|---|---|---|---|---|---|
| C2 | `Swizzler`, `MaskSwizzler`, `Sampler`, `SampledCodec`, `AndroidCodec`, `AndroidCodecAdapter`, `ScalingCodec` | `SkSwizzler.cpp`, `SkMaskSwizzler.cpp`, `SkSampler.cpp`, `SkSampledCodec.cpp`, `SkAndroidCodec*.cpp` | C1 | L | — |
| C3 | BMP (standard, mask, RLE) + WBMP | `SkBmp*Codec.cpp`, `SkWbmpCodec.cpp` | C2 | M | `Codec_wbmp*` (3), `Codec_bmp`, `Codec_Bmp_b511820841`, `Codec_bmp_indexed_colorxform`, … (~7) |
| C4 **(S)** | `skia-rust-zlib` inflate (Chromium `inflate.c`/`contrib/optimizations/inflate.c` semantics, `inftrees`, `inffast`, adler32, crc32) + codec-diff cases | zlib@646b7f56 | C0b | M | (C5) |
| C5 **(S)** | `skia-rust-libpng` read path: `png.c` (CRC, chunk checks), `pngpread.c`, `pngrutil.c` (IHDR, PLTE, tRNS, gAMA, cHRM, sRGB, iCCP, sBIT, IDAT, IEND, unknown chunks), the transforms in §1.1, `pngget.c`/`pngset.c` parts used, `png_set_option`, error/longjmp → `Result` | libpng@d5515b5b | C4 | L | (C6) |
| C6 | `PngCodec`, `PngCodecBase`, `PngCompositeChunkReader`, `PngChunkReader`; `IcoCodec` (with the registry lookup for PNG) | `SkPngCodec*.cpp`, `SkIcoCodec.cpp` | C2, C5 | L | `CodecTest` PNG/ICO (~15), `BadIcoTest` 3, `IndexedPngOverflowTest`, `InvalidIndexedPngTest`, `CodecExactReadTest`, `CodecPartialTest` ~3, `AndroidCodecTest` 4, `LibpngCodec_f16_trc_tables` |
| C7 **(S)** | Lazy images: core `ImageGenerator`, `ImageLazy`; codec `CodecImageGenerator`, `make_from_encoded`, `deferred_from_encoded_data`, `codecs::deferred_image`; test/GM helpers (`GetResourceAsImage`, `DecodeUtils`) | `SkImageGenerator.cpp`, `SkImage_Lazy.cpp`, `SkImage_LazyFactories.cpp`, `SkCodecImageGenerator.cpp`, `SkImageGenerator_FromEncoded.cpp`, `tools/DecodeUtils.cpp`, `tools/Resources.cpp` | C6 | M | `ImageGeneratorTest` 2, `ImageTest` (core/codec, ~4), `MipMapTest` 2, `Skbug6389`, `ImageNewShaderTest` 1; **GMs: up to ~125 PNG-only GMs (file scan)**, e.g. `bmp_filter_quality_repeat`, `readpixels`, `tilemodes*`, `complexclip`, `drawatlas`, `vertices`, `mesh`, `savelayer`, `texelsubset`, `alpha_image`, `colorwheel*` (PNG part) |
| C7b | GM sweep: port/flip the PNG-only GMs whose other dependencies have landed; record blockers as reasons | gm/* | C7 | M (sweep) | as above |

### Wave C-B: GIF (Wuffs)

| ID | Task | Sources | Depends | Size | Unlocks |
|---|---|---|---|---|---|
| C8 **(O)** | `skia-rust-wuffs`: transcribe the generated C of `base` (status, io buffers, pixel config/buffer, the swizzler and blend functions for `BGRA_NONPREMUL`/`RGBA_NONPREMUL`/`BGR_565` from indexed/`BGRA`), `lzw`, `gif`. **Coroutines:** each `wuffs_gif__decoder__*` coroutine keeps its C resume point (`coro_susp_point`) and spilled locals (`private_data.s_<func>`) in the struct, and the Rust function is a `loop { match pc { … } }` over the same labels. That is a mechanical one-to-one transcription, which matters for suspension behaviour | `wuffs-v0.3.c` (GIF/LZW/BASE sections) | C0b | L | (C9) |
| C9 | `WuffsCodec` (one-pass and two-pass, the latter drawing through `Canvas`), `gif_decoder`; `SkAnimatedImage` (android utils) | `SkWuffsCodec.cpp`, `src/android/SkAnimatedImage.cpp` | C2, C8 | M | `GifTest` 9, `CodecTest` GIF (~8), `Wuffs_seek_and_decode`, `CodecPartialTest` ~4, `CodecAnimTest` ~3, `AnimatedImageTest` 2; GMs `animatedGif`, `flight_animated_image`, `AnimCodecPlayerExif_required.gif`, `filterindiabox` |

### Wave C-C: JPEG

| ID | Task | Sources | Depends | Size | Unlocks |
|---|---|---|---|---|---|
| C10 **(S)** | `skia-rust-libjpeg` decompress, baseline: `jdapimin/jdapistd/jdmaster/jdinput/jdmarker/jdhuff/jdcoefct/jddctmgr/jidctint` (incl. `jpeg_idct_NxN`), `jidctred`, `jdsample` (fancy + box), `jdcolor` (+ `jdcol565.c`, RGBA/BGRA/gray/CMYK/YCCK), `jdmainct/jdpostct`, `jdatasrc`, `jdicc`, `jutils`, `jerror` → `Result`, `jcomapi`; `jpeg_crop_scanline`, `jpeg_skip_scanlines`, `raw_data_out` | libjpeg_turbo@e14cbfaa | C0b | XL | (C12) |
| C11 | Progressive (`jdphuff`, block smoothing in `jdcoefct`, `buffered_image`) and arithmetic (`jdarith`, `jaricom`) | same | C10 | M | (C12) |
| C12 | `JpegCodec`, `JpegDecoderMgr`, `JpegSourceMgr`, `JpegUtility`, `JpegMetadataDecoderImpl` (ICC, EXIF; gainmap hooks stubbed), YUV planes (`getYUVAPlanes`); `Exif`, `TiffUtility`; core `yuva_info`/`yuva_pixmaps` if missing | `SkJpeg*.cpp`, `SkExif.cpp`, `SkTiffUtility.cpp` | C2, C10, C11 | L | `CodecTest` JPEG (~10), `ExifTest` 7, `YUVTest::Jpeg_YUV_Codec`, `ImageGeneratorOrientationTest`, `ImageIsOpaqueTest`, `SamplingTest`, `PDFJpegEmbedTest` (decode part); GMs `grayscalejpg`, `orientation_4xx` (6) + `respect_orientation_jpeg`, `repeated_bitmap_jpg`, `ducky_yuv_blend`, `async_rescale_and_read_dog_*` (with async rescale), `wacky_yuv_formats_imggen`, `blurs` (5), `copy_to_4444` (2), `compositor_quads`, `runtimeshader` JPEG parts |
| C13 | Metadata: core `xml::{parser, dom}` (Q4), `Xmp`, `JpegXmp`, `JpegSegmentScan`, `JpegMultiPicture`, core `GainmapInfo`, `HdrMetadata`, `HdrAgtm` (parse/serialize/math, no shader); gainmap decode in `JpegCodec` and `PngCodecBase` (`gmAP`/`gdAT`) | `SkXMLParser.cpp`, `SkDOM.cpp`, `SkXmp.cpp`, `SkJpegXmp.cpp`, `SkJpegSegmentScan.cpp`, `SkJpegMultiPicture.cpp`, `SkGainmapInfo.cpp`, `SkHdrMetadata.cpp`, `SkHdrAgtm*.cpp` | C12, C6 | L | `SkXmpTest` 7, `SkJpegXmpTest` 3, `JpegGainmapTest` 6, `PngGainmapTest` 2, `HdrMetadataTest` parse/math (~8 of 11) |

### Wave C-D: WebP

| ID | Task | Sources | Depends | Size | Unlocks |
|---|---|---|---|---|---|
| C14 **(S)** | `skia-rust-libwebp` decode: `dec/*` (VP8 frame/tree/quant/bit reader, VP8L, alpha, idec, io, buffer, webp), `dsp` C files (`dec.c`, `lossless.c`, `filters.c`, `upsampling.c`, `yuv.c`, `alpha_processing.c`, `rescaler.c`, `dec_clip_tables.c`), `utils` (huffman, color cache, rescaler, quant levels dec, thread as sequential), `demux` | libwebp@845d5476 | C0b | XL | (C15) |
| C15 | `WebpCodec` (frames, Skia-side blending through the raster pipeline, subset, scaling), `webp_decoder` | `SkWebpCodec.cpp` | C2, C14 | M | `WebpTest`, `CodecTest` WebP 3, `Codec_webp_animated_image_rewind`, `CodecAnimTest`/`AnimatedImageTest` rest (~4), `ExifTest` 1; GMs `stoplight_animated_image`, `AnimCodecPlayerExif_*.webp` (2), `async_rescale_and_read_rose` (3, with async rescale) |

### Wave C-E: encoders

| ID | Task | Sources | Depends | Size | Unlocks |
|---|---|---|---|---|---|
| C16 **(S)** | zlib deflate (Chromium `deflate.c` with ANZAC hash, `trees.c`, levels/strategies) + libpng write path; `PngEncoderImpl`/`Base`, `Encoder`, `encode::icc` (`SkICC`) | zlib, libpng, `SkPngEncoder*.cpp`, `SkEncoder.cpp`, `SkICC.cpp` | C6 | L | `EncodeTest` PNG (~6), `Codec_pngRoundTrip`, `Codec_EncodeICC`, `ImageTest` encode (3), `CodecRecommendedTypeTest`, `PngGainmapTest` encode, `SerialProcsTest`, `SerializationTest`; GMs `encode-srgb-png`, `encode`, `image.cpp` (7), `encode-platform`; fuzz `PNGEncoder` |
| C17 | libjpeg-turbo compress (`jcapimin/jcapistd/jcmaster/jcinit/jcparam/jcmarker/jccolor/jcsample/jcprepct/jcmainct/jccoefct/jcdctmgr/jfdctint/jchuff/jcphuff/jcarith/jdatadst/jcicc`); `JpegEncoderImpl`, `JPEGWriteUtility`, `JpegGainmapEncoder` | libjpeg-turbo, `SkJpegEncoderImpl.cpp`, `SkJpegGainmapEncoder.cpp` | C12, C13 | L | `EncodeTest` JPEG (3), `EncodedInfoTest`, `YUVTest::Jpeg_YUV_Encode`, `JpegGainmapTest` encode 3, `ExifTest` 1; GMs `encode-srgb-jpg`, `encode-alpha-jpeg`, `jpg-color-cube`, `image_shader` (3); fuzz `JPEGEncoder` |
| C18 **(S)** | libwebp encode (lossy + lossless + `sharpyuv` if reached), `mux`, `anim_encode`; `WebpEncoderImpl` | libwebp `enc/*`, `mux/*`, `dsp/*enc*.c`, `SkWebpEncoderImpl.cpp` | C14, C16 | XL | `EncodeTest` WebP (4); GMs `encode-srgb-webp`, `encode-color-types-webp-*` / `encode-opaque-*` (4 names); fuzz `WEBPEncoder` |

### Wave C-F and later

| ID | Task | Depends | Size | Unlocks |
|---|---|---|---|---|
| C22 | DM image sources (§9.4) in the GM harness | C7, C9, C12, C15 | M | 217 `dm-image` once goldens exist (Q2) |
| C19 | `GainmapShader`, AGTM shader (effects) | C13, SkSL S18 | M | `GainmapShaderTest` 4, `HdrMetadataTest` shader (~3) |
| C20 | `AnimCodecPlayer` + Skottie hook | C9, C15, Skottie | S | `CodecAnimTest::AnimCodecPlayer` |
| C21 | Benches: `DecodeBench` 6, `EncodeBench` 28, `WebpBlendBench` 1 | C-A…C-E | M | 35 (perf gate) |
| C-R1 | `SkPngRustCodec` + `SkPngRustEncoderImpl` over `png =0.18.1` (+ Skia's patch, vendored; Q1/Q5) | C1, C2, C16 | L | 53 + 3 + `PngRustHdrMetadataRoundTrip` + fuzz `PNGRustEncoder` |
| C-R2 | `SkJpegRustCodec`/encoder over `zune-jpeg =0.5.16-rc1`, `jpeg-encoder =0.6.0` + Skia's `experimental/rust_jpeg/ffi` Rust | C12, C13 | M | 18 |
| C-R3 | Port Skia's Rust `rust/exif` and `rust/icc` parsers (no external crates) | C12 | S | `RustExifTest` 8, `RustIccTest` 5 (skcms) |
| C-R4 | BMP/ICO via `image` (blocked on a crates.io release with rev `5d0418d0`) | C3 | S | 34 |

### Dependency summary

```
C0, C0b ─┐
C1 ──► C2 ──► C3
         ├──► C6 ◄── C5 ◄── C4 ◄── C0b        ──► C7 ──► C7b (PNG GM sweep)
         ├──► C9 ◄── C8                        (GIF)
         ├──► C12 ◄── C11 ◄── C10              ──► C13 ──► C17, C19 (needs SkSL S18)
         └──► C15 ◄── C14                      ──► C18
C6 ──► C16 ──► C18;   C7+C9+C12+C15 ──► C22 (dm-image, waits for goldens)
```

Parallelism: C4/C5, C8, C10/C11 and C14 (the library ports) depend only on C0b, so they can
start on day one alongside C1/C2. The first visible pass is C3 (BMP/WBMP tests). The first big
lever is C7.

---

## 11. Risks and open questions

| # | Risk | Plan |
|---|---|---|
| R1 | **libjpeg-turbo's control flow is the hard part**, not its arithmetic: suspension in the data source (`jdatasrc` + Skia's `SkJpegSourceMgr` returning `FALSE` to suspend), `jpeg_skip_scanlines` with fancy-upsampling context rows, `buffered_image` for truncated progressive files (`Codec_jpeg_decode_progressive_*`). | C10 is Sonnet and keeps libjpeg's module structure (`jpeg_decompress_struct` → a struct with sub-module state structs; method pointers → enums dispatched with `match`, since the "virtual" methods only switch among a few known implementations). `codec-diff` cases cover every truncation point. |
| R2 | **arm64 NEON vs C** for libjpeg-turbo colour conversion/upsampling (and libwebp/libpng NEON). We believe they are identical for the paths Skia uses (§4), but haven't verified it. | `codec-diff` NEON build on the aarch64 runner (C0b). A mismatch becomes a `Tier::Neon` variant of that one function. |
| R3 | **x64 SIMD vs C** in libwebp (SSE2/SSE4.1), libpng, Wuffs and zlib: believed exact. | `codec-diff` SSE build (one-time check, recorded in this note). Only an actual difference would require a ported SIMD variant. |
| R4 | **skcms most likely ran SKX in the oracle** on every x64 tier (its CPUID check ignores `SKIA_ORACLE_CPU_CAP`), while `skia-rust-skcms` is scalar. skcms is built with `-ffp-contract=off` and uses no estimate instructions, so the kernels should agree, but nobody has compared them on codec transforms (ICC-tagged JPEG/PNG, `Codec_ColorXform`, `gradient_*` resources). | Extend `oracle/skcms-diff` with an `-DSKCMS_FORCE_SKX` build on an AVX-512 host (or the GitHub runner if it has AVX-512). If they differ, skcms needs per-tier kernels: a separate task, flagged to the coordinator. |
| R5 | **libm in codec math** (gainmap/HDR `log2`/`exp2`/`pow`, libwebp encoder `pow`). | Use the UCRT-exact libm (PR #87); move it to `skia-rust-base` so the port crates can reach it. |
| R6 | **Global caches.** Skia caches decoded lazy images and YUV planes in `SkResourceCache`. Per-image caching changes nothing visible in pixels, but `ImageCacheTest`/`YUVCacheTest`/purge tests inspect the global cache. | Per-image cache now. Those tests get the reason "needs SkResourceCache port" until a task ports the cache (shared with mipmaps, `images.md`). |
| R7 | **WebP lossless encoder floats** (entropy estimates with SSE2 variants, `VP8LFastLog2`) can change the bitstream between C and SSE2. Pixels stay lossless, but sizes and bytes can differ, and `Encode_WebpOptions` compares sizes. | Port the C, compare with `codec-diff` SSE build. If the oracle's SSE2 sums differ, port the SSE2 summation order as the scalar code (it's just an order of float adds). |
| R8 | **Resources missing on CI** make every codec test skip and look green. | C0 adds resources to the test matrix. `inventory verify` already refuses `passing` for tests that didn't run. Make sure a skipped resource test counts as "didn't run". |
| R9 | **Port size.** libjpeg-turbo (~20k C lines touched in total), libwebp (~30k with the encoder) are bigger than any port so far. | Port only Skia's paths (§2), one subsystem per PR, each checked by `codec-diff` before any Skia-level test. |
| R10 | **Wuffs transcription.** Generated C is verbose (about 6k lines for GIF/LZW + 3k of base used here), and its coroutine resume logic has to come over exactly. | C8 on Opus. One `match pc` per coroutine, labels named after the C `case` numbers, and `codec-diff` short-read cases at every byte offset of small GIFs. |
| R11 | **`Codec` lifetime and `Send`** may not fit every skia-safe call site (`from_stream` with a non-`Send` reader). | Document in API_MAPPING. Readers that aren't `Send` can be buffered into `Data` first. |
| R12 | **Encoders' byte equality with Skia** is not tested by codec tests, so a subtly non-faithful deflate or Huffman optimizer would go unnoticed until PDF. | `codec-diff` covers deflate and the JPEG compressor bytes too, so encoders are held to the same standard as decoders. |

Open questions for the coordinator:

1. **Q1 — Rust-backed codecs (121 entries).** Amend PLAN §2 "Codecs" to put Skia's Rust PNG/JPEG
   codecs and its Rust EXIF/ICC parsers in scope as wave C-R, exclude BMP/ICO-Rust (34) until
   `image` publishes the needed revision, and record the dependencies like skrifa? (Recommended:
   yes. These tests are self-checking and the crates are Skia's own implementation.)
2. **Q2 — `dm-image` goldens.** The Windows oracle host is gone. Options: (a) leave 217 entries
   `todo` until a host exists (default); (b) build a CPU-only Linux DM (no Dawn, no fonts) and
   render `--src image` for `8888`/`565`/`f16`. First compare a sample of `--src gm` outputs
   against `goldens-m156` to prove the Linux build is equivalent. Codec pixels are integer
   code, but the colour and pipeline stages run per tier, and skcms SKX vs Windows matters (R4).
   Recommended: (b) once C-A…C-D land, with the equivalence check as a gate.
3. **Q3 — crate split.** Five library crates plus `skia-rust-codec` (recommended), or one
   codec crate with `third_party/` modules? The split costs 5 crate names on crates.io
   (reserve them) and makes licensing exact.
4. **Q4 — XML.** A small safe tokenizer crate (`xmlparser`, MIT/Apache, `forbid(unsafe_code)`)
   with expat's callback semantics re-created in `SkXMLParser`, or a port of expat? Recommended:
   the crate. Revisit if an SVG test exercises malformed-XML behaviour that differs.
5. **Q5 — patched `png`.** Skia applies `captured-chunks.patch` to `png 0.18.1`. Vendor the
   patched crate in-tree (publishable as `skia-rust-png-patched`?) or reimplement the captured
   chunks on top of `png`'s public API (deviates from Skia's Rust glue)? Recommended: vendor.
6. **Q6 — `SkCodecs::Register` as process-wide state.** Accept the `RwLock` registry (needed
   by 4 serial tests and by ICO's PNG lookup), or exclude those 4 tests and pass decoders
   explicitly only? Recommended: accept, documented like the strike cache.

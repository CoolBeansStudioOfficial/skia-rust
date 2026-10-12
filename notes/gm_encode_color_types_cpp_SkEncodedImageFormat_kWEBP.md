# gm/encode_color_types.cpp::SkEncodedImageFormat::kWEBP (and `#2`)

Status: `failing`. Registered as `def_variants_gm!` (three variants: `encode-color-types-webp-lossless`,
`encode-opaque-color-types-webp-lossless`, `encode-gray-color-types-webp-lossless`; `#2` is the
`webp-lossy` (quality 80) set). The id passes only when every variant matches on every tier
(`check::check_gm_set`).

## What matches
- `8888` and `565` match on every checkable tier for every variant. The opaque and gray variants match
  on `f16` too. Variants that skip on a config match the oracle's skip.
- `f16/encode-color-types-webp-lossless` and `f16/encode-color-types-webp-lossy` (the normal variant)
  are the only mismatches, on every tier (40 comparisons per id, `gm-verify --no-diffs`).

## Where the bytes differ (cpu-x64-sse2 golden, `target/gm-diffs/...` raw dumps)
The GM is 896x128 f16 (8 bytes/px). Blocks of 128 px: pair 1 (opaque) src and decode at x 0 and 128,
pair 2 (premul, rainbow-gradient.png) src at x 320 and decode at x 448, pair 3 (unpremul) at 640 and 768.
- Lossless: only the decoded premul pair differs: 402 px, all with alpha 1.0, the largest byte diff 247.
  Example, px (460,4): ours `[1, 0.00784, 0.75293, 1]`, golden `[1, 0.00784, 0.74854, 1]`. Our source
  px (332,4) is `0.75293` in both, so the golden's decoded value sits about one F16 ulp (or one 8-bit
  level) below ours.
- Lossy: only the decoded premul pair differs: 1185 px, a few levels off (e.g. (496,4): ours
  `[6,38,79]` vs golden `[4,36,87]`, F16 bytes).
- The source pairs and the unpremul pair match exactly. 8888 matches everywhere, including the
  premul→unpremul readPixels (non-NEON builds run it through the pipeline).

## Checked
- `SkWebpEncoderImpl.cpp`: F16 is not an 8888 type, so `preprocess_webp_picture` converts with
  `readPixels(RGBA8888 unpremul)`. `crates/skia-rust-core/src/convert_pixels.rs` follows
  `SkConvertPixels.cpp` (`convert_with_pipeline` for F16). The `unpremul` stage matches
  `SkRasterPipeline_opts.h#L2710`. With alpha 1 it is the identity, so it cannot explain the
  lossless diff.
- `to_unorm` (`store_8888`) rounds to nearest; `convert_to_alpha8`'s truncation applies only to alpha8.

## Hypothesis (unverified)
The golden's F16 canvas holds a value that is not on the 8-bit grid (0.74854 = 1533/2048), so the
difference is not only in the WebP input. A draw or conversion stage on the F16 path (the premul pair
only) gives a different F16 result than ours. That points at the raster pipeline on F16 (drawing a
premul source, or the readPixels pipeline), not at the codec. It needs a raster-pipeline dump of the
oracle for `encode_color_types` (`cargo xtask oracle rp-dump`), which needs the oracle host: it is
gone, so this stays open.

## Next step
Get the oracle's RP stages for the premul pair (or an F16 `readPixels` / `drawImage` rp-diff case with
premul input) and find the first stage where the values diverge. Do not tweak the GM.

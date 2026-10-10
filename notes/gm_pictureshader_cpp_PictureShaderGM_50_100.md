# Picture shader GMs: 565 and f16 mismatches (shared note)

Covers the picture shader GMs that were ported on branch `port/picture-shader` and do not match
their goldens yet:

| Manifest id | Config | Differing pixels (of total) |
|---|---|---|
| `gm/pictureshader.cpp::PictureShaderGM(50, 100)` | 565 | 226 of 2,030,000 |
| `gm/pictureshader.cpp::PictureShaderGM(50, 100, true)` | 565 | 226 of 2,030,000 |
| `gm/pictureshader.cpp::PictureShaderGM(50, 100, false, 0.25f)` | 565 | 35 of 2,030,000 |
| `gm/pictureshader.cpp::pictureshader_persp` | f16 | 27 of 23,650 |
| `gm/pictureshadercache.cpp::PictureShaderCacheGM(100)` | 565 | 3 of 10,000 |
| `gm/pictureshadertile.cpp::PictureShaderTileGM` | 565 | 134 of 480,000 |

8888 (RGBA oracle tiers) matches for every picture shader GM. The f16 config matches for every
GM except `pictureshader_persp`. Our 565 output is the same bytes on the scalar and the SSE/AVX
tiers (one hash), and the same on the ML3/ML4 model tiers (a second hash), and the goldens are
consistent across tiers, so this is not a tier or host issue.

## Diagnostic pass (one, done)

The diff images (`target/gm-diffs/cpu-x64-sse2/565/pictureshadercache.png`, then numeric
comparison of the `.raw` and `.golden.raw` files) show:

- 565: the differing pixels are all anti-aliased edge pixels of the green circle over the white
  background. Each differs by one 5-bit unit in the red and blue channels only, and the green
  channel is equal. Example, pixel (45, 10): ours `(21, 63, 21)`, golden `(20, 63, 20)`. The
  tile image itself is RGBA8888 for a 565 destination in both (`CachedImageInfo::Make`, 6 bits
  per channel is at most 8), and the 8888 output of the same scene matches, so the difference
  comes from the store to the 565 destination: the rounding of the red and blue values when a
  partially covered image-shader colour is blended over the destination.
- f16 `pictureshader_persp`: 27 pixels, all in the second (picture shader) strategy, between
  x = 113 and x = 200, around the glyph edges of the perspective-transformed text. The tile
  image is `RGBA_F16Norm` here. The 8888 and 565 configs of the same GM match, so the
  difference is in the f16 sampling of the image in a perspective matrix.

## Hypothesis

Neither is picture-specific. Both show an image-shader result (sampled from the tile image) being
stored or blended into a non-8888 destination with one-unit rounding differences. The likely
places are the 565 store stage (`store_565`, its `to_unorm` rounding) and the lowp versus highp
choice for the image stage in f16. The picture shader GMs are the first ported GMs that go
through that path with partial coverage over a non-8888 destination.

Next step: `cargo xtask oracle rp-dump` on the 565 `pictureshadercache` GM (the oracle's
raster-pipeline stages), and compare with ours at the pixel (45, 10). The oracle host no longer
exists, so this needs the committed dumps, or a note to the maintainer that the dump has to be
regenerated.

## Not the cause

- The tile sizes and the image info: they are computed from the same matrices as in
  `SkPictureShader::CachedImageInfo::Make`, and the 8888 output matches.
- The local matrix: the `PictureShaderGM` variants with and without the local matrix wrapper
  differ by the same pixel count (226 each). This was not checked region by region.
- Not checked: whether the bitmap-shader half of the same scenes matches in 565. Only the
  whole-image counts were compared.

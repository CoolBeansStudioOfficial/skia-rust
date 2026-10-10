# gm/wacky_yuv_formats.cpp WackyYUVFormatsGM(kFromGenerator), golden wacky_yuv_formats_imggen

Status: failing (one attempt). The GM is ported in `tests/gm/src/gm/wacky_yuv_formats.rs` and
registered with `#[ignore]`.

What was done: create_splat/add_arc/make_bitmap, extract_planes, create_YUV (all ten formats),
YUVAPlanarConfig, the generator (YUV to premultiplied RGBA through the origin matrix and
per-pixel plane lookups), the labels and the draw loop. The planes are copied into
`YUVAPixmaps::from_external_pixmaps`.

Result (`gm-verify --match WackyYUVFormatsGM`): every tier and config mismatches the golden.
The diff image (`target/gm-diffs/cpu-x64-sse2-rgba/8888/wacky_yuv_formats_imggen.png`) shows
the grid, labels and original row matching; the one band that differs is the 4:4:4 interleaved
row (AYUV or Y410, the 5th or 6th format row) across all columns.

Hypotheses, not checked:
- Y410 (RGBA1010102) packing: `(a << 30) | (v << 20) | (y << 10) | u` and the 10-bit rounding
  of `Y/U/V` (`round(x/255*1023)`), and the read-back of RGBA1010102 through the generator's
  1x1 `read_pixels` conversion.
- AYUV: the raw `SkColorSetARGB(A, V, U, Y)` stored as a native u32 in an RGBA8888 bitmap.

Next step: dump the first differing pixel of that row from the oracle goldens and compare
with the planes produced by `create_yuv` for that format; check the two hypotheses above
before changing the arithmetic.

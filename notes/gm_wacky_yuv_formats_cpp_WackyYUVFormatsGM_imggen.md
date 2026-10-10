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

## Second attempt (branch port/yuva-cpu3): not resolved, still failing

What was done: seeded the golden cache per the preamble, built gm-verify in release, and ran
`gm-verify --match WackyYUVFormatsGM` with diffs. Every tier and config still mismatches (ours
`2dd72380...` vs golden `68ceb28e...` for 565; `45aefe1a...` vs `5de20777...` for f16). The 8888
diff PNG for `cpu-x64-scalar-rgba` shows three panels (ours | golden | diff) at 11415x1451, so each
panel is 3805 px wide. The raw dumps are 3805x1451 RGBA8888.

Findings:
- The diff PNG shows only one band mismatching (the 4:4:4 interleaved format row).
- Comparing the raw dumps pixel by pixel gives 311949 differing pixels over 194 rows
  (first at x=1, y=220: ours `cccccc`, golden `000000`), so the raw comparison does not match
  the single band the PNG shows. Either the raw dump is not laid out as the PNG panels, or the
  mismatch is wider than the diff image shows. This was not resolved.

Not checked (still open): Y410 packing and the 10-bit rounding, AYUV channel order, and the
alpha/color type per plane. The YUVAPixmaps plane change in this branch did not alter the
pixel values of this GM (it only changes plane views; `from_external_pixmaps` still copies).

Next step: confirm the raw dump layout against the golden's dimensions (look at how gm-verify
writes `.raw` for multi-panel output), then dump the first mismatching pixel of the 4:4:4 row
from both sides and check the Y410/AYUV packing against `SkYUVAPixmaps` plane formats.

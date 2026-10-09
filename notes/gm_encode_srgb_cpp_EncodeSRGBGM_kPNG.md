# gm/encode_srgb.cpp::EncodeSRGBGM(SkEncodedImageFormat::kPNG) (encode-srgb-png)

Status: failing (registered with `#[ignore]`).

## Where the mismatch is

The earlier note said "a few RGB565 pixels differ". That is only the last surface. The same
pixels differ in every surface (the GM is drawn into N32, RGBA_F16 and RGB565 canvases):

- 565 surface: 30 pixels, all in one cell.
- 8888 surface: 174 pixels, all in one cell, every one off by +1 in blue (ours 243, golden 242).
- f16 surface: 174 pixels, the same cells.

The differing cells are grid row 4 (y 512..639), columns 0 and 1. Row 4 is the `RGBAF16` colour
type with `Premul` alpha, drawn from `images/color_wheel.png`. Every other cell matches, including
the `RGBAF16` unpremul and opaque rows and the RGB565 row. The PNG encoder is not the cause: the
decode of that cell is already different.

## Pipeline trace (first divergence)

The first divergent value is the RGBA_F16 premultiplied decode of `color_wheel.png` (source
pixel (66,0) = (0, 255, 242, 253)), blue channel.

- Ours (`make_codec` -> `get_pixels` into RGBA_F16 premul, scalar and SSE2 tiers): blue = 0x3b88
  (1928/2048 = 0.941406). Alpha = 0x3bef (2031/2048). This is the skcms float premultiply
  (242/255 * 253/255 = 0.941594) followed by the software `Half_from_F` truncation. The C++
  `skcms` path gives the same value (`modules/skcms/skcms.cc` puts the `premul` op after the
  destination encode, for `PremulAsEncoded`; `Half_from_F` truncates without F16C).
- Golden, inferred from the golden outputs, not dumped: blue = 0x3b87 (1927/2048). The PNG
  encoder then writes 16-bit blue 62177 (ours 62211), and the golden 8-bit outputs follow from
  that: 8888 = 242 (ours 243), F16 = 1943/2048 (ours 1951/2048).

`SkConvertPixels` (F16 premul to R16G16B16A16 unorm unpremul, through the highp pipeline
`unpremul` and `store_16161616`) was checked by hand against the C++ and gives 62211 from 0x3b88,
so it is consistent. The divergence is at the decode, not at the encode.

Golden value at this pixel is consistent with 0x3b87 across three independent observations
(8888, F16 and 565), and with no other half value.

## Experiments (not kept)

All on the scratch branch, reverted, not committed.

1. 8-bit rounded premultiply (`round(v*a*255)/255`) in skcms `Op::Premul`: the mismatch grew to
   674 pixels in the 8888 surface, with errors in both directions. Rejected.
2. Premultiply by the alpha after a half-precision round trip (`a_h = trunc_half(a)`): the 8888
   mismatch fell from 174 to 7 pixels, and the 565 and f16 PNG hashes still do not match.
   Remaining 7 pixels are one step off in both directions (for example (62,518) ours 249 golden 250),
   so this is not the mechanism. It is not kept, because it is a tuned value and not a port.
3. Also round-tripping the colour channels through half before the multiply (in addition to 2):
   same 7 pixels. Colour round trip is not the cause.

None of these is a faithful port, so none is committed.

## Hypotheses for the next attempt

- The golden decode may use a path that stores or multiplies alpha in half precision (the
  `a_h` result of experiment 2 is the closest so far). The C++ path that does this has not been
  found in `src/codec` or `modules/skcms`. Check `SkRasterPipeline` usage in the codec and
  `SkSwizzler` for an F16 premultiply stage, and the libpng vs Rust PNG codec choice
  (`SK_CODEC_DECODES_PNG_WITH_LIBPNG` vs `..._WITH_RUST`, `src/codec/SkPngRustCodec.cpp` premul at
  lines 317-333).
- The remaining 7 pixels after experiment 2 point at a second rounding step, possibly in the
  draw of the decoded image (the 16-bit to 8-bit high-byte truncation in the N32 PNG decode).

## Next step

Needs an oracle per-stage dump of the RGBA_F16 premul decode of `images/color_wheel.png` at
(66,0), and of the PNG 16-bit rows the encoder writes for that cell. The oracle host no longer
exists, so this cannot be captured here. The JPEG GM (`notes/gm_encode_srgb_cpp_EncodeSRGBGM_kJPEG.md`)
has the same root cause and should be fixed by the same change.

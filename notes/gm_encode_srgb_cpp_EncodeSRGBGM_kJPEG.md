# gm/encode_srgb.cpp::EncodeSRGBGM(SkEncodedImageFormat::kJPEG) (encode-srgb-jpg)

Status: failing (registered with `#[ignore]`).

Attempts: one full run of `gm-verify`, with the PNG variant's diff as the reference.

Observed: all 90 cell and config comparisons mismatch, and every cell draws an image. The JPEG
encoder is verified byte-exact against libjpeg-turbo (`tests/expected/encode.txt`), and the same
decode path that feeds this GM also feeds `encode-srgb-png`, which differs only in a few RGB565
pixels. The cause is therefore most likely the source decode (`color_wheel.jpg` through
`SkCodec::getPixels` to each colour type), compounded by the JPEG encode/decode of the result.

Next step: fix the `encode-srgb-png` mismatch first (see its note). Then re-check this GM.

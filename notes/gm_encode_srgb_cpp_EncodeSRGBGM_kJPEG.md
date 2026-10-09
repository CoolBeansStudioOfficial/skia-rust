# gm/encode_srgb.cpp::EncodeSRGBGM(SkEncodedImageFormat::kJPEG) (encode-srgb-jpg)

Status: failing (registered with `#[ignore]`).

## Where the mismatch is

It is not every config. The 8888 surface (scalar tier) differs in 1332 pixels, with a maximum
channel difference of 3 (767 pixels by 1, 543 by 2, 22 by 3). Only two grid cells differ, both in
row 4 (y 512..639), columns 0 and 1: cells (0,4) with 725 pixels and (1,4) with 607. Row 4 is the
`RGBAF16` premultiplied row, the same cell as `encode-srgb-png`.

So the JPEG GM has the same root cause as the PNG GM: the RGBA_F16 premultiplied decode of
`images/color_wheel.png` gives blue 0x3b88 where the golden has 0x3b87. The JPEG encoder is not
involved in the mismatch: it is verified against libjpeg-turbo, and the other rows, which go
through the same encoder, match. The whole-image hash differs in every config only because those
two cells are part of every surface.

Details of the first divergent value, the experiments already run (none kept), and the next
step are in `notes/gm_encode_srgb_cpp_EncodeSRGBGM_kPNG.md`. Fixing that decode should fix this GM
too. The next attempt must then re-check this GM (`--match EncodeSRGBGM`) before un-ignoring.

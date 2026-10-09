# gm/encode_srgb.cpp::EncodeSRGBGM(SkEncodedImageFormat::kPNG) (encode-srgb-png)

Status: failing (registered with `#[ignore]`).

Attempts: one full run of `gm-verify` with diff images.

Observed: every colour-type and alpha cell is drawn (720 draws, no missing images), so the
mismatch is in pixel content. In the diff for `565/encode-srgb-png`, only a few pixels differ, in
the RGB565 cells. The PNG round trip is lossless, so the suspects are the decoder's conversion to
the destination format (`SkCodec::getPixels` to RGB565 from `color_wheel.jpg`, and the
`makeColorSpace(sRGB)` variants), not the PNG encoder.

Hypothesis (not verified): the RGB565 output of the JPEG decode path (libjpeg-turbo's
`jdcol565.c` with `JDITHER_NONE`, or `SkSwizzler`) differs by one step in a few pixels.

Next step: compare `getPixels` output for `images/color_wheel.jpg` in RGB565 against the oracle's
dumps, per pixel, before touching the encoder.

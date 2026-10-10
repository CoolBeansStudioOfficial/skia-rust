# gm/readpixels.cpp::ReadPixelsGM

Status: failing (ported 1:1 in tests/gm/src/gm/readpixels.rs, `#[ignore]`d).

What was tried: one full port. make_raster_image decodes images/google_chrome.ico through
Codec::get_image(info with 64x64, colour type, Premul); draw_image reads each image back with
Image::read_pixels into 8888 / BGRA / RGBA F16 (unpremul and premul) in the wide (ProPhoto
parametric), sRGB and small-gamut colour spaces, then draws the bytes via raster_from_data.

Mismatch: all 45 checks (8888 and 565, every tier) differ. The diff image
(target/gm-diffs/cpu-x64-scalar-rgba/8888/readpixels.png) is mostly within tolerance-free
equality, but has differing pixels in the cells whose source is the F16 decode (rows 2, 5 and 8
of the 3x3 colour-space x source-type grid) and in some BGRA-source cells.

Hypothesis (not verified): the ICO decode into RGBA F16 (codec colour conversion on the
codec -> F16 path) or Image::read_pixels from F16 into 8888 with the wide-gamut colour space
differs from Skia m156 (SkConvertPixels / SkRasterPipeline gamut transform). The first divergence
is the (wide gamut, RGBA_F16 source) cell, so compare the F16 codec output first, then the
read_pixels colour transform for that cell, before changing code.

Also not ported: ReadPixelsCodecGM (needs a colour-space canvas; onDraw returns kSkip without one)
and ReadPixelsPictureGM (SkImages::DeferredFromPicture is not on main).

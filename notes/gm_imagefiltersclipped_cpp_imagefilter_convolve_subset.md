# gm/imagefiltersclipped.cpp::imagefilter_convolve_subset

Status: ported 1:1 (`tests/gm/src/gm/imagefiltersclipped.rs`), `#[ignore]`d, status `failing`. The diff shows the top half (the 3x3 `MatrixConvolution`, clamp, convolveAlpha, crop) identical on every tier, and the bottom half (`Blur(10, 10, kMirror, nullptr, crop)`, drawn at y = 90) blank in skia-rust but blurred in the golden.

Attempts
1. Straight port of the GM and of the two filters (`image_filters::matrix_convolution`, `image_filters::blur` with a mirror crop). gm-verify, every tier: mismatch. The diff panels are golden, ours, difference.

Hypothesis (not yet checked): the legacy-tiling path of `BlurImageFilter` (`blur_filter.rs`: `legacy_tile_mode`, `apply_crop` with `TileMode::Mirror` and a `None` input) produces an empty result when the crop has no explicit input, so the blur contributes nothing. Next step: evaluate `image_filters::blur(10, 10, Mirror, None, Some(crop))` on a 160x90 source and compare with `SkCropImageFilter.cpp` `onFilterImage`/`applyCrop` for kMirror.

The matrix convolution filter itself is not the cause (its half matches on every tier).

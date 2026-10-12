# gm/animated_image_orientation.cpp::AnimatedImageGM("images/flightAnim.gif", "flight", 20, ...)

Status: failing (GM `flight_animated_image`). The `stoplight` variant of the same file passes on every checkable tier.

## What was tried
1. Full port of SkAnimatedImage (`crates/skia-rust-codec/src/animated_image.rs`) and of the GM
   (`tests/gm/src/gm/animated_image_orientation.rs`). The six AnimatedImageTest.cpp tests pass.
2. Fixed `AndroidCodec::get_android_pixels` to pass the real destination into `handle_frame_index`
   (it passed an empty slice, so a kRestoreBGColor previous frame panicked in `zero_rect`).
3. Fixed `AndroidCodec::compute_sample_size` for animated WebP: C++ sets `desiredSize = origDims`
   when `getFrameCount() > 1` (the Rust port returned 1 without it). This made `stoplight` pass.

## Mismatch (release gm-verify, 8888 on the RGBA tiers, 565/f16 too)
- Canvas is 3216x3216 (T = 402 cells of 402 px). Rows 0..2 (scales 1.25, 1.0, 0.75) and rows 4..6
  match the golden exactly. Rows 3 and 7 (scale 0.5) differ in every cell (about 11% of pixels,
  max channel difference 67, all opaque).
- Scale 0.5 is an exact 2:1 downscale of a 320x240 frame: the codec decodes at full size
  (sample size 1 on both sides; GIF is an adapter codec whose `onGetScaledDimensions` is the
  default, so `computeSampleSize` returns 1), and SkAnimatedImage draws it with `fMatrix = Scale(0.5)`
  and `SkSamplingOptions(kLinear)`.
- Using the golden's own row 1 (scale 1.0, identical to ours) as the exact source frame:
  - our output == round(2x2 box average) (what bilinear at the exact 2:1 midpoint should give): 0 mismatches vs ours, 17826 vs golden.
  - golden == source pixel (2x, 2y) exactly (nearest-like): **0 mismatches vs golden**.
  - (2x+1, 2y+1): 17086 mismatches vs golden.
- Picture path (`makePictureSnapshot`) and image path (`getCurrentFrame`, drawn with nearest) give
  identical diff counts, so the divergence is the scaled bilinear draw of the frame, not the
  frame content or the snapshot.

## Hypothesis
The golden's exact 2:1 linear downscale picks the top-left texel of each 2x2 block. Either Skia's
bilinear sampling for an exact half-scale matrix lands on texel centres (weight 1 on one texel,
i.e. the sample coordinate is 2x+0.5 in texel-centre space, a half-texel offset from ours), or the
raster path downgrades to a nearest-style sample for an integer inverse scale. The stoplight GM
scales by 14/29 (not an exact 0.5), so it does not exercise this case, which is why it matches.
Candidates to check in the Rust port: the `S32_alpha_D32_filter_DX` equivalent (fixed-point
fraction and weight rounding at fx = 0.5 exactly) and the raster pipeline's bilinear stage
(`bilinear_nx`/`bilinear_px` offsets) for scale-translate matrices with inverse scale 2.

## Next step (needs an oracle run, none is available on this host)
`cargo xtask oracle rp-dump` for `flight` at 8888/565 on the scalar tier, to get the first stage where
the sample coordinate or weights diverge. Without it, do not change code by trial and error.

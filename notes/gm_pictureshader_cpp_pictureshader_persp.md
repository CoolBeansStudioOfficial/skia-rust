# gm_pictureshader_cpp_pictureshader_persp

`pictureshader_persp` mismatches in **f16 only** (8888 and 565 match). The other five picture
shader GMs were fixed (565 root cause below); this one remains.

## What is known

- Differing pixels per golden tier class, same GM: scalar 24, sse2..avx 27, ml3 11, ml4 9. They
  are all in the second (picture shader) strategy, x = 113..200, at the glyph edges, and only the
  G channel differs (text is green over black; R = B = 0, A = 1). The differences are 1..4 half
  ulps, in values like 0.0013 (absolute 1e-6..6e-5). Positions differ per tier class, so the
  cause is a float operation whose rounding is tier dependent, not a constant offset.
- The tile is `RGBA_F16Norm` for the f16 config, as in `CachedImageInfo::Make`. Making the tile
  `RGBA_F32` instead gives 410 differing pixels, so the golden tile really is F16-quantised.
- The pipeline we build: `seed_shader, matrix_perspective, bilinear_setup, bilinear_nx,
  bilinear_ny, decal_x_and_y, gather_f16, check_decal_mask, accumulate, ...` (4 taps),
  `move_dst_src, load_f16_dst, srcover, store_f16`.
  The `matrix_perspective` context is
  `[0.5715951, 0, -62.875458, 0.020785054, 0.57157683, -9.64424, -0.0041990005, 0, 1.370981]`.
- Perturbing each entry of that matrix by +-1 or +-2 ulps (hack, since removed) never gets to 0
  differences on any tier (baseline 24/27/11/9; entries 0, 2, 6, 8 are at their minimum, so they
  are right to the ulp; entries 3, 4, 5 barely matter). So a one-ulp error in a single matrix
  entry is not the cause.
- libm is not the cause: `tanf(pi/6)`, `sinf(0.008)`, `cosf(0.008)` from glibc are the correctly
  rounded values (checked against Python doubles), the inputs of `SkM44::Perspective` / `Rotate`.
- The direct strategy (same text, drawn straight into the f16 canvas) matches.

## Hypotheses left (none tested)

1. A tier-dependent float difference in the bilinear weights: the diffs are about one ulp of the
   sample coordinate (3.8e-6 at x in [32, 64)) times the texel difference, which is the size seen.
   Compare `bilinear_setup`/`bilinear_nx`/... and `accumulate` with the oracle with `rp-diff`
   cases that feed perspective-sized coordinates (the existing cases may not).
2. The F16 tile content: text drawn into the F16Norm tile by the A8 mask blit
   (`uniform_color, clamp_01, load_f16_dst, lerp_u8, store_f16`; `lerp_u8` is FMA on ml3/ml4).
   A one-half-ulp difference in a few tile texels would show as these few pixels. Check by
   dumping the tile and comparing to the f16 golden of a GM that draws the same text directly
   at the tile scale.
3. `rcp_precise` / the division in `matrix_perspective` on the non-FMA tiers.

The first thing to try is hypothesis 2 (cheap): dump the tile texels around the first differing
pixel (index 6639 = x 189, y 30 in the 215-wide canvas; the tile coordinate follows from the inverse matrix above).

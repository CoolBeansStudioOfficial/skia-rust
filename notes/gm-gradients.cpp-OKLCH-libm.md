# gm/gradients.cpp: `gradients_color_space_tilemode`, `gradients_color_space_many_stops`

Both GMs interpolate in OKLCH. Status: `failing` (8888 and 565 differ in a few pixels, f16 differs in
several thousand, on every tier).

## What matches

- Every other gradient GM in `gm/gradients.cpp` and friends passes on every tier (linear, radial, sweep,
  conical, premul and unpremul interpolation, hard stops, dithering, local matrices, perspective).
  So the stage ports (`gradient`, `evenly_spaced_*`, `xy_to_*`, `mask_2pt_conical_*`, `apply_vector_mask`),
  tiling, the stop clean-up and `SkColor4fXformer` agree with Skia, including `css_hcl_to_lab` /
  `css_oklab_to_linear_srgb` as far as these GMs reach them.
- The goldens are not off by a systematic error: `many_stops` 8888 (sse2-rgba oracle) differs from ours in
  181 of 250,000 pixels, every difference is +-1 in one channel.

## Hypothesis: the oracle's `cbrtf` (and maybe `atan2f`) differ from ours by ~1 ulp

The first divergence is at the stops. `SkGradientBaseShader.cpp` converts each stop to OKLCH with
`std::cbrtf` and `atan2f` (`lin_srgb_to_oklab`, `lin_srgb_to_okhcl`). The clamped region of
`gradients_color_space_tilemode` (f16, Scalar tier) is the round trip of one stop, and the golden
pixels fix its tiny green/blue channel to about 1e-7 absolute:

| end | channel | ours (f16 bits) | golden |
|---|---|---|---|
| blue stop, `(5,5)` | g | 4091 | 4094 |
| yellow stop, `(130,5)` | b | 5106 | 5100 |

Searching +-3 ulp around our `cbrtf` results (and +-4 ulp on the hue) reproduces both values with many
combinations, so the data does not identify the oracle's function. What I tried (scratch probes, not kept):

- `cbrtf` as `(float)cbrt((double)x)`, `pow` in `float` and `double`, `expf(logf(x)/3)`,
  `expf(logf(x)*(1/3))`, `exp2f(log2f(x)/3)`, one Newton step from either: only
  `exp2f(log2f(x)/3)` makes both 8888 and 565 match exactly (0 differing pixels in `many_stops`) and fixes
  the blue end in f16, but the yellow end (`l`: -1 ulp, `s` or `m` still 1 ulp off) and so f16 still differ.
- `atan2f` as `(float)atan2((double)y,(double)x)`, `atanf(y/x)` with quadrant fix, `2*atan(y/(r+x))`: no
  change in any output.

The Windows oracle links the UCRT, whose `cbrtf` is not correctly rounded; ours (`f32::cbrt`, the host libm)
is. Reproducing it bit for bit needs the UCRT algorithm (or oracle values of `cbrtf` for the 3 + 3 + ...
arguments), which cannot be obtained now. Everything else about these GMs is ported 1:1.

## To continue

Run `cargo xtask oracle` on the oracle host with a harness that prints `cbrtf`/`atan2f` of the stop
conversions (l, m, s of red, green, blue, yellow) and compare with `f32::cbrt`; then port the oracle's
implementation into `gradient_base_shader.rs` (`xyzd50_to_lab`, `lin_srgb_to_oklab`, hue). The Lab/LCH
interpolation spaces call the same functions.

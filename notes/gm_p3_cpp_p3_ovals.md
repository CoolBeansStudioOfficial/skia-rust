# gm/p3.cpp::p3_ovals

Status: `failing` (ported, registered, `#[ignore]`d).

## What was tried

1. Faithful port (`tests/gm/src/gm/p3.rs`): four Display-P3 red shapes (a circle, an oval, a
   butt-capped dashed circle with a 10 px stroke, and a rotated oval), each painted with
   `Paint::set_color4f({1, 0, 0, 1}, Display P3)`. The `compare_pixel` checks are not ported: they
   print on a mismatch and never draw.

## Results (gm-verify --no-diffs --match p3_ovals)

- Every config and every tier mismatches (8888 on sse2, sse41, ml3 and ml4; 565 on all tiers).
  Example: 8888 ours `7e478426...`, sse2 mismatch.

## Hypothesis

Not confirmed. The shapes are the same kind as the passing GMs (AA circles and ovals, dash path
effect), so the likely cause is the colour path: `set_color4f` with a non-sRGB colour space
(`ColorSpaceXformSteps` from Display P3 to sRGB, then the pipeline's colour stage), or the
handling of an out-of-gamut result after the transform. It could also be in the dash or the
rotated oval. Only one attempt has been made.

## Next step

Needs a fresh oracle run: `cargo xtask oracle rp-dump <tier> p3_ovals`, compared with our
raster-pipeline stages for the first (circle) draw, which isolates the colour conversion from the
shape. The Windows oracle host no longer exists, so this cannot be done on the current hosts.

# gm/postercircle.cpp::PosterCircleGM()

Status: `failing` (ported, registered as `PosterCircleGM()`, `#[ignore]`d).

## What was tried

1. Faithful port of `PosterCircleGM` (`tests/gm/src/gm/postercircle.rs`): the 12 poster
   images are drawn onto one reused raster surface and snapshotted per angle; the M44 model
   matrix is `Translate(300, 225) * proj * Translate(0, ringY) * RotateY * Translate(0, 0, 200)`
   with `proj.setRC(3, 2, -1/800)`; posters are drawn with `drawImageRect` (linear sampling,
   alpha 0.7). `onAnimate` is not ported: DM never animates, so the time stays 0.

## Results (gm-verify --no-diffs --match PosterCircleGM)

- `565`: matches on every oracle tier.
- `8888`: matches on sse2, sse41, ml3 and ml4 (the model tiers). The `scalar` tier differs only
  from the wasm proxy golden (`cpu-x64-scalar-rgba`), which never decides a verdict.
- `f16`: mismatches on every oracle tier, for all tiers that the host checks (scalar, sse2,
  sse41, ml3, ml4). Example: ours `76a0f653...` vs golden `48950dc5...` (sse2).

## Hypothesis

The divergence is only in the f16 pipeline, and the 3D (perspective) image draw is the main
new ingredient compared with the passing GMs. The likely candidates are the f16 highp path of the
image sampling stage for a perspective-mapped `drawImageRect`, or the f16 blend of the
0.7-alpha image draw. This was not confirmed.

## Next step

Needs a fresh oracle run: `cargo xtask oracle rp-dump <tier> postercircle` (for the f16 config)
to compare the raster-pipeline stages against ours. The Windows oracle host no longer exists, so
this cannot be done on the current hosts. Start there when the oracle is available again.

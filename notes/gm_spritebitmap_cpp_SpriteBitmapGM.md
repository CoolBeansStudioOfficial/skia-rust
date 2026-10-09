# gm/spritebitmap.cpp::SpriteBitmapGM

Status: `failing` (ported, registered, `#[ignore]`d).

## What was tried

1. Faithful port (`tests/gm/src/gm/spritebitmap.rs`): an N32 bitmap (100x100, blue with a red
   anti-aliased circle) drawn four times with `drawImage` at (10, 10), (10, 130), (10, 250),
   (10, 370) and a shifted 20 px gap, with and without a clip and an 8-sigma blur image filter.

## Results (gm-verify --no-diffs --match SpriteBitmapGM)

- `8888` and `565`: match on every oracle tier.
- `f16`: mismatches on every oracle tier (scalar, sse2 and the rest). Example: ours
  `97b59dff...` vs golden `3c091d15...` (sse2).

## Hypothesis

The same f16-only mismatch shows up in `gm/postercircle.cpp::PosterCircleGM()`, which also draws
N32 images (surface snapshots) onto the F16 surface. The common factor is an N32 image drawn onto
an F16 raster surface (the source conversion or the sampling stage of the f16 pipeline), not the
blur or the clip. Only one of the two has been tried; that is not enough to confirm it.

## Next step

Needs a fresh oracle run: `cargo xtask oracle rp-dump <tier> spritebitmap` for the f16 config,
compared with our raster-pipeline stages for `drawImage` of an N32 bitmap onto an F16 canvas.
The Windows oracle host no longer exists, so this cannot be done on the current hosts.

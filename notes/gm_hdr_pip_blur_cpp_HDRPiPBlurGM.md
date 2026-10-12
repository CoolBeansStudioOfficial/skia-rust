# gm/hdr_pip_blur.cpp::HDRPiPBlurGM

Status: `failing`. The GM is ported (`tests/gm/src/gm/hdr_pip_blur.rs`, `kGM_Mode`, 640x360), together with
the two APIs it needed: `Canvas::new_surface` (`SkCanvas::makeSurface`, already on main as
`RasterCanvas::new_surface`) and `Surface::make_temporary_image` (`SkSurface::makeTemporaryImage`, which
for a raster surface is the default `onMakeTemporaryImage`, i.e. the snapshot).

## Mismatch
Every comparison mismatches (45 of 45 on the checkable tiers): 565, f16 and 8888 (RGBA oracle variants on
an RGBA host).
- `565`: 109540 of 230400 pixels differ, by 1 level in one 5/6-bit channel (green in the sample).
- `f16`: 129471 of 230400 pixels differ, max |diff| 0.0159 (about 4 levels of 8 bits). The differences are
  spread over the whole frame (both the blurred background and the PiP), so it is not one stage's edge.
- Sample, 565, (0,0): ours `0x31a6`, golden `0x3186` (green 13 vs 12). (320,180) matches exactly.

## What the GM exercises
Background `images/yellow_rose.png` (sRGB) and PiP `images/mandrill_512.png` converted to PQ/Rec.2020
(`makeColorSpace`), drawn into a RGBA8888 surface in Rec.2020 transfer / Display P3 gamut
(`SkColorSpace::MakeRGB(kRec2020, kDisplayP3)`), saturation 1.5 colour-matrix filter on the paint,
an RRect clip, `makeTemporaryImage`, an 8 px clamped blur (`SkImages::MakeWithFilter`) drawn at alpha 0.9.
The colour-space transfers (sRGB and PQ) and the blur are the candidate sources of a systematic
per-pixel rounding difference.

## Checked
- Structure and arguments follow `gm/hdr_pip_blur.cpp` line by line; sigma is `32 * 640 / 2560 = 8`.
- The GM does not depend on the kSample/bench modes (DM runs `kGM_Mode`).

## Next step (needs the oracle or a committed dump)
Find the first divergent stage: the `rp-dump` of the oracle for `hdr-pip-blur` (`cargo xtask oracle
rp-dump`), then the sRGB→PQ and Rec.2020 transfer stages (raster pipeline `from_srgb`/`pq` tail
approximations) and the blur stage separately. The oracle host is gone, so this stays open.

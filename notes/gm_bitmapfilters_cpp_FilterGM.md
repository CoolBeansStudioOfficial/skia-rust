# gm/bitmapfilters.cpp::FilterGM

Status: `failing` on this host (RGBA N32); pixels otherwise identical.

## What was ported

`FilterGM` (gm/bitmapfilters.cpp) in `tests/gm/src/gm/bitmapfilters.rs`: the 2x2 `make_bm`
bitmap copied to 4444 and 565, the three rows of `drawImage` with nearest, linear and dithered
sampling, the row labels drawn with the portable font, and the 0.5-alpha second set.

## Result

`gm-verify --match bitmapfilters::FilterGM`: 565 and f16 fail on every tier, and 8888 fails on
five tiers. The diagnostic pass with diffs (565, scalar) shows the differing pixels are only in
the label column, and after the label fix only in the `BGRA_8888` label row (x 12 to 35,
y 201 to 209: 157 pixels).

The label is `ToolUtils::colortype_name(kN32_SkColorType)`. Skia gives `BGRA_8888` on the
Windows oracle (the goldens) and `RGBA_8888` on an RGBA host such as this Linux one. The port
prints the host's name, which is what Skia prints on this host.

## Next step

Nothing in the GM is left to port. The only difference is the N32 label, which matches the
goldens only on a BGRA host (Windows, where the oracle runs). It stays `failing` on this host.
That it matches on a BGRA host is expected from the analysis above but has not been run here.

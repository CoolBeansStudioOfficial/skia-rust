# tests/graphite/sparse_strips/PolylineTest.cpp::SparseStrips_Polyline_Integrated

Status: `failing` (the test is `#[ignore]`d). Skia's own test disagrees with Skia's own
implementation at the pin (`chrome/m156`, `95ee33d7ec77`); the port follows the pin.

## What fails

`Flatten::processPaths` (`src/gpu/graphite/sparse_strips/Flatten.cpp#L361-L490`, ported in
`crates/skia-rust-gpu/src/graphite/sparse_strips/flatten.rs`) gives, for a contour whose curves are
all culled (entirely above, right of or below the viewport), the points
`[start, NaN, last, start, NaN]`: the culled curve leaves `needStartPt` set, and the next `moveTo`
(or the end of the path) closes the contour with `appendPoint(lastPt); appendPoint(startPt)`.

The test expects `[start, NaN]` ("the implicit close points match the moveTo points exactly, so
Polyline drops the close point via deduplication"):

- `// Culling`: `polyline.count() == 6` (the port has 15 points, from 5 per contour).
- `// Interleaved valid geometry with culling and simplification`: `polyline.count() == 13` (the
  port has more points, because the culled cubic contour adds its last and start points).

`FlattenTest.cpp` (`TestCulling`, which passes in the port) documents the behaviour the code at the
pin has: a single culled curve is "Start pt + Cull NaN + Last pt + Start pt (5 points)".

## Cause

Upstream `main` (`google/skia`, `src/gpu/graphite/sparse_strips/Flatten.cpp`) has since changed
`processPathsImpl`, in a commit after the pin: it takes `width` and `height` and closes contours with

```cpp
auto closeContour = [&]() {
    if (!closed && lastPt != startPt) {
        SkPoint closePts[2] = {lastPt, startPt};
        if constexpr (kShouldCull) {
            if (!is_completely_culled<2>(closePts, width, height)) {
                if (needStartPt) { polyline->appendPoint(lastPt); }
                polyline->appendPoint(startPt);
            }
        } else { /* as before */ }
    }
    closed = true;
    needStartPt = false;
};
```

(used at the `moveTo`, the `close` verb and the end of the path). `PolylineTest.cpp` is identical at
the pin and in `main`. Trying that change in the port (and reverting it, as it is not in the pin)
made `SparseStrips_Polyline_Integrated` pass in full, so the closing edge of a culled contour is the
only difference.

`main` also changed `estimate_lines_from_quad` (`scale` without `x2 - x0`), `is_finite_simd` (masks
the sign bit) and `estimateLinesFromQuadSimd` (`a0`/`da` of collinear quads), which no ported test
depends on.

## To unblock

Move the pin past that commit (or port `closeContour` with a `Port of:` link to the newer commit),
un-`#[ignore]` the test and set the entry to `passing`.

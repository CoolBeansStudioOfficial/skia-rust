# gm/arcto.cpp::parsedpaths

Status: ported 1:1 (`tests/gm/src/gm/arcto.rs`), `#[ignore]`d. 47 pixels (of 250000) differ on every tier
and config, +-1 in single colour channels along the edges of two curves in the cell at (300,300)
(x 328-341 / y 318-376, and x 373-378 / y 357-394), plus one at (254,425). The random SVG strings, the parsed
paths (verbs, points, conic weights) and everything else are identical.

Attempts
1. Straight port; the random spec generation matches (the image is otherwise identical, so `SkRandom`,
   `str_append_scalar`/`%.8g` and the SVG parser agree with Skia for ~150 random path strings).
2. Dumped the three paths of the differing cell (verbs/points/weights) and checked the parse
   (`SkParse::FindScalar` = `(float)strtod`, already faithful).

Hypothesis
The paths contain SVG `A` arcs with huge radii and odd rotations (e.g. `A79.49,74.68,496.9,0,0,...`,
`a27.4,1.54,179.14,...`). `SkPathBuilder::arcTo(rad, angle, ...)` uses the float libm functions
`SkScalarATan2`, `SkScalarTan`, `SkScalarCos` (and `SinSnapToZero`/`CosSnapToZero`), whose results are not
guaranteed to match between the oracle host (Windows UCRT) and ours (Rust `f32` methods on the host libm).
A one-ulp difference in a conic control point is enough to move AAA coverage by 1 in a few pixels.
Smaller `A` cases (the `arcto`, `bug583299`, `bug593049` GMs, rounded rects) pass bit-exactly.
Needs bit-exact UCRT float trig, or an oracle dump of the parsed points, to confirm.

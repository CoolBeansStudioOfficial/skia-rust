# gm/dashing.cpp::longpathdash

Status: ported 1:1 (`tests/gm/src/gm/dashing.rs`), `#[ignore]`d. 5 pixels differ from the golden on every
tier and config (8888 RGBA on scalar: (247,21) (248,22) (248,23) (284,40) (284,41), differences of 1-2 in
a colour channel on anti-aliased dash edges). Everything else in the image is identical.

Attempts
1. Straight port. Checked every float/double promotion against the C++ (`(float) sin(a) * x`,
   `sin(a + 3.141592 / 3)` computed in double, float loop counters `a += 0.03141592f`, `i += 0.05f`).
2. Compared the diff pixels against the geometry: they sit on the dash segments of a handful of the
   ~7000 generated lines, i.e. a line endpoint that differs by one ulp.

Hypothesis
The endpoints come from the double precision `sin`/`cos` libm calls (Windows UCRT on the oracle host vs the
host's libm / Rust `f64::sin`). One-ulp differences in a couple of the thousands of calls move an endpoint
by one float ulp, which flips a rounding in the dash/stroke/AAA arithmetic for a few pixels. PORTING.md §libm
lists exactly this risk. A fix needs a bit-exact UCRT `sin`/`cos`; nothing in the rasterizer is implicated
(all other dashing GMs pass bit-exactly).

Not tried: dumping the 2*N endpoint floats from the oracle (no oracle host available).

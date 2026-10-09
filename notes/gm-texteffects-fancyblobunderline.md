# gm/texteffects.cpp::fancyblobunderline

Status: failing (`tests/gm/src/gm/texteffects.rs`), registration `#[ignore]`d.

Observed (gm-verify, release, this host): every config mismatches on every tier, in both the
8888 and 565 and f16 color types. Example: 8888 scalar, ours `0c92db73…`, golden `95ea6d02…`.
Diff images are written to `target/gm-diffs/<tier>/<color type>/fancyblobunderline.png`.

What the diff shows (from the 8888 sse2 diff image)
- The sans-serif rows match. Only small differences remain at descenders in the serif rows
  (visible at the `p` and `Q` tails in the diff image).
- The differences are a few pixels per glyph, with no visible offset in the blob or underline
  positions.

Not yet known
- Root cause. Candidates: the intercept computation in `TextBlob::getIntercepts` (double vs float
  arithmetic on the serif glyph bounds), or the serif font's outline rounding on the path branch.
- Pixel counts per row were not measured. The next step is to diff the intercept arrays for the
  serif rows against the C++ values, following docs/PORTING.md §12 (rp-diff), before changing code.

Attempts: none beyond the initial port. The GM is ignored so CI stays green until the cause is found.

Update (port/gm-rsx-fix): the RSXform shader fix (make_post_inverse_lm in device.rs) did not change this
GM; it is unrelated to RSX glyph runs and still mismatches on every tier.

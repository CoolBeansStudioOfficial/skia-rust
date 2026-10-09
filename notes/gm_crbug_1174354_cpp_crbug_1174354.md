# gm/crbug_1174354.cpp::crbug_1174354

Ported 1:1 in `tests/gm/src/gm/crbug_1174354.rs` (saveLayer with bounds, sweep gradient in
`kMirror` tile mode, `SaveLayerRec::fBackdrop` with a clamped `Blur` crop filter). Registered and
`#[ignore]`d.

Result with `gm-verify`: the `8888` and `565` configs match on every tier. The `f16` config
mismatches on every tier (scalar, sse2, sse41, ml3, ml4), with one hash per config: ours
`cb9c59d4…`, golden `99e82b73…`.

Attempts:
1. Ported as above. Verified the neighbouring pieces in isolation: `crbug_1313579` (backdrop with a
   clamped blur, all configs including `f16`) and `gradients::*` (sweep gradients, `f16` included)
   pass. So the `f16` problem is in the combination used here, not in either piece alone.

Hypothesis: the `saveLayer` with explicit `bounds` (`outsetRect`, outset 10px from the source rect)
combined with a backdrop filter reads or clips the `f16` layer differently from Skia's
`SkCanvas::internalSaveLayer` in the `f16` path. Not confirmed. The next step is
`cargo xtask oracle rp-dump` for `f16` on this GM, which needs an oracle host, so it is not
possible in the current environment.

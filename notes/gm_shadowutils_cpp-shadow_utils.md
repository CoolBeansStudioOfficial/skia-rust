# gm/shadowutils.cpp: shadow_utils, shadow_utils_occl, shadow_utils_gray, shadow_utils_directional

Status: failing (ignored in `tests/gm/src/gm/shadowutils.rs`). `shadow_utils_gaussian_colorfilter`
passes on every tier.

## Update (port/shadows-full)

- The concave helpers (`is_simple_polygon`, `offset_simple_polygon`,
  `triangulate_simple_polygon`) are now used from the full `skia_rust_core::utils::poly_utils`
  module. The convex-only subset module was removed. `compute_concave_shadow` and
  `stitch_concave_rings` are ported from `SkShadowTessellator.cpp#L568-L738`, so concave paths no
  longer fall back to blur.
- Root cause for `shadow_utils_directional` (fixed): in `compute_convex_shadow`, the collapsed-umbra
  color was computed as `SkPMLerp(kUmbraColor, kPenumbraColor, ratio)` in C++, but the Rust passed
  `UMBRA_COLOR` twice, so the lerp was a no-op. Small shapes (the 0.25-scale row) got the wrong
  umbra color. After the fix, directional matches on 8888 and 565 for every tier, and on f16 for
  the scalar and sse2 tiers.
- Remaining: `shadow_utils_directional` f16 on the ml3 and ml4 tiers differs by sparse single-pixel
  specks (ours hash `a7f4926e...`, golden `91750c73...`). The same ours-hash is produced for both
  ml3 and ml4. Not isolated. Next step: compare the f16 raster-pipeline stage dumps for one of the
  small shapes against `cpu-x64-sse41-rt-ml3`.
- `shadow_utils`, `shadow_utils_occl`, `shadow_utils_gray`: concave shapes now render through the
  real tessellator. The diff is a per-pixel residual, not a missing feature (the star and dumbbell
  are drawn in the right places). Not yet isolated. Next step: dump the concave ring vertices
  (`stitch_concave_rings` output) and compare with the C++ vertex list for the star path.

## Mismatch data (`gm-verify --match shadow_utils`, release)

- `shadow_utils` / `_occl` / `_gray`: mismatch on all tiers.
- `shadow_utils_directional`: see the update above.

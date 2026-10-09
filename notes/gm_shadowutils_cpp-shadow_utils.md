# gm/shadowutils.cpp: shadow_utils, shadow_utils_occl, shadow_utils_gray, shadow_utils_directional

Status: failing (ignored in `tests/gm/src/gm/shadowutils.rs`). `shadow_utils_gaussian_colorfilter`
passes and the `tests/ShadowTest.cpp` entries pass.

## What is ported

- `SkDrawShadowInfo` (`crates/skia-rust-core/src/draw_shadow_info.rs`): the metrics, spot and
  directional parameters, `GetSpotShadowTransform`, `GetLocalBounds`.
- `SkShadowTessellator` (`crates/skia-rust-core/src/shadow_tessellator.rs`): convex ambient and
  spot meshes, including the Ganesh curve subdivision (`GrPathUtils`) and the scalar Wang's
  formula (`skgpu::wangs_formula`) with the identity transform.
- `SkShadowUtils` and `SkDevice::drawShadow` (`crates/skia-rust-core/src/shadow_utils.rs`):
  `DrawShadow`, `GetLocalBounds`, `ComputeTonalColors`, the vertex draw with the Gaussian color
  filter, and the blur fallback. The public API is `skia_rust_core::utils::shadow_utils`.
- `SkGaussianColorFilter` (`crates/skia-rust-core/src/gaussian_color_filter.rs`).
- `SkInsetConvexPolygon`, `SkIsConvexPolygon`, `SkComputeRadialSteps`
  (`crates/skia-rust-core/src/poly_utils.rs`).

## What is not ported (why the grid GMs cannot pass)

- Concave polygon helpers: `SkIsSimplePolygon`, `SkOffsetSimplePolygon`,
  `SkTriangulateSimplePolygon` (`SkPolyUtils.cpp`, about 1,000 lines of C++ beyond the convex
  subset). `shadow_utils`, `shadow_utils_occl` and `shadow_utils_gray` draw a star and a dumbbell.
  The tessellator reports failure for concave paths, so those shadows fall back to the blur path,
  which is not what Skia draws.
- Skia's `SkResourceCache` of tessellations is skipped. The uncached path (`key == nullptr` in
  `draw_shadow`) computes the same vertices and translation as a cache hit, so output should match.
  This was reasoned from the code, not measured.

## Mismatch data (`gm-verify --no-diffs --match shadow_utils`)

- `shadow_utils_directional` (convex rounded rects only, so the concave gap does not apply):
  - 8888 tier scalar: ours `ed7cac3498844dca2ef07a3b2e113258f54c2c31fe906bb830dc89e00e5fa532`,
    golden `8b9d00f8d107b8bf3f92ac656bf55f0b8f76ef38a8e913cadd545e7562cf6ff9`.
  - 565 tier: ours `059790e6b3cb5250a8a767fdbda48024ca3405e6b284138d4d8791989057defd`,
    golden `66f7ae39777f071e5fe2d0989ba9655455aa01e6088be0061c12f31f06d31376`.
  - The same ours-hash appears on every tier of a given byte format, so the difference is not
    tier-specific.
- `shadow_utils`, `shadow_utils_occl`, `shadow_utils_gray`: mismatch on all tiers, expected while
  the concave helpers are missing.

## Unit checks that do pass

- `tests/ShadowTest.cpp::ShadowUtils` and `::ShadowBounds` pass with the port
  (`cargo test -p skia-rust-tests --lib shadow_`). They check the tessellator's success and failure
  contract and bounds containment, not pixels.

## Blocked on data

- Golden images: `objects-m156.tar` returns HTTP 401 from this container, and no PNG dump tool exists
  in the checkout, so pixel diffs against the golden are not available. The `hashes-m156.json`
  file was copied from another worktree's cache.
- Oracle dumps: none for shadows in `oracle/`.

## Hypotheses for the directional mismatch (not verified)

1. The vertex draw. `draw_mesh` uses `Blender::mode(Dst)` and `skip_color_xform = true`, with
   the Gaussian color filter composed after `Blend(color, Modulate)`. The Gaussian GM passes, so
   the filter itself is probably right, but the vertex-color and blender interaction in
   `draw_vertices` is untested.
2. The conic to quad path (`handle_conic_mapped`) and the Wang's formula point counts for the
   rounded corners. A different segment count changes the mesh outline slightly.
3. The directional spot offset: `get_directional_params` and the `GetSpotShadowTransform` branch
   that the directional GM uses with a non-identity matrix.

Next step for whoever continues: obtain the golden objects (or the oracle dumps), render the
ambient mesh alone for the translation-only case, and compare its vertex list with the C++
`SkShadowTessellator` output for the same rounded rect. That isolates hypotheses 1 and 2 without
guessing.

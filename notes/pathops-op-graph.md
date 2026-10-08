# PathOps op graph (slice 3): status and hand-over

Branch `port/pathops-3` (draft PR #93). Manifest entries flipped to `passing`:
`tests/PathOpsSimplifyTest.cpp::PathOpsSimplify` and `::bug_513001309`.

## Ported (crates/skia-rust-pathops/src)

| File | Skia source (m156) | State |
|---|---|---|
| `op_state.rs` | SkPathOpsTypes.h (SkOpGlobalState), arenas and ids | complete |
| `op_span.rs`, `op_segment.rs`, `op_contour.rs`, `op_angle.rs`, `op_curve.rs` | SkOpSpan, SkOpSegment, SkOpContour, SkOpAngle, SkPathOpsCurve | complete for the paths the ops use; some helpers unused until the tight-bounds and builder slices (`#![allow(dead_code)]` at the top of each file) |
| `op_coincidence.rs` | SkOpCoincidence.cpp | complete |
| `op_winding.rs` | SkPathOpsWinding.cpp (sortableTop, rayCheck, windingSpanAtT) | complete |
| `path_writer.rs` | SkPathWriter.cpp (partials, assemble) | complete |
| `op_edge_builder.rs` | SkOpEdgeBuilder.cpp | complete |
| `op_add_intersections.rs` | SkAddIntersections.cpp, SkIntersectionHelper | complete |
| `op_common.rs` | SkPathOpsCommon.cpp (AngleWinding, FindChase, SortContourList, HandleCoincidence) | complete |
| `op_simplify.rs` | SkPathOpsSimplify.cpp | complete (`pub fn simplify`) |
| `op_op.rs` | SkPathOpsOp.cpp (findChaseOp, bridgeOp, gOpInverse/gOutInverse, OpDebug, Op) | complete (`pub fn op`) |

Public surface so far: `skia_rust_pathops::{op, simplify}` and `path_op::PathOp`.

Not yet: `OpBuilder` (SkOpBuilder.cpp), `tight_bounds` (SkPathOpsTightBounds.cpp), `as_winding`
(SkPathOpsAsWinding.cpp), the `PathOpsExt` extension trait on `Path` and its re-export in the
`skia-rust` facade, and the `docs/API_MAPPING.md` note for the deviation.

## Tests

| Test file | Status |
|---|---|
| PathOpsSimplifyTest.cpp | 2 of 2 DEF_TESTs pass (PathOpsSimplify: all 470 table entries; bug_513001309). Flipped. |
| PathOpsOpTest.cpp | partial: `tests/src/unit/path_ops_op_test.rs` ports testIntersect1/2, testUnion1/2, testDiff1/2, testXor1/2, testOp1d, testOp2d (10 of about 450 functions). `PathOpsOpPartial` passes. DEF_TEST(PathOpsOp) stays `todo`. |
| PathOpsTightBoundsTest.cpp | not ported (needs `tight_bounds`) |
| PathOpsAsWindingTest.cpp | not ported (needs `as_winding`) |
| PathOpsBuilderTest.cpp | not ported (needs `OpBuilder`) |

Helpers in `tests/src/unit/path_ops_extended_test.rs`: `inner_simplify`/`test_simplify`/
`test_simplify_fail` and `test_path_op`.

Deviations, to report with the PR:

1. `innerPathOp` only compares the result with the region boundary when `reporter->verbose()`.
   That comparison (`comparePaths` with `SkRegion::op` and `getBoundaryPath`) is not ported. Our
   `test_path_op` checks success only, which is exactly what Skia checks without `--verbose`.
2. `testSimplify` in our port compares the result on every run (Skia compares only when verbose).
   This is stricter than Skia, so it cannot hide a failure Skia would report.
3. `comparePaths` for Simplify uses Skia's bitmap comparison (`pathsDrawTheSame`, `MAX_ERRORS` 9),
   not a pure region comparison; a region comparison gave 133 false mismatches on curved cases.
4. `Simplify` no longer calls `set_phase(Walking)`: that call is `DEBUG_VALIDATE` only in Skia.

## Verification status of the op port

The op is checked only by the success assertions in the 10 ported PathOpsOp cases and by the
Simplify suite (which exercises the shared graph code). No geometric output check has run yet.
The verbose-only region comparison (deviation 1) is the check that would catch a wrong result, so
it is the first thing to add when porting PathOpsOpTest in full.

Resolved from the previous hand-over: `SK_NaN32` is `i32::MIN` and `SK_MinS32` is `i32::MIN + 1`
(SkMath.h); segment points are copied into `Segment::pts` (the edge builder does not write through
its array after the segments are built).

## Next steps

1. Port the rest of `tests/PathOpsOpTest.cpp` (functions, the `tests[]`, `failTests[]` and
   `repTests[]` tables, the `ops` arrays and `path_edit`). Flip only the tests seen passing.
2. Port `SkOpBuilder.cpp`, `SkPathOpsTightBounds.cpp`, `SkPathOpsAsWinding.cpp`, then the
   `PathOpsExt` trait and facade re-export, and `docs/API_MAPPING.md`.
3. Port `PathOpsTightBoundsTest`, `PathOpsAsWindingTest`, `PathOpsBuilderTest`.
4. Remove the module-level `clippy` and `dead_code` allows once the callers exist, and fix the
   remaining lints they hide where they do not mirror Skia.

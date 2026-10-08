# PathOps op graph (slices 3 and 4): status and hand-over

Slice 3 (PR #93, `port/pathops-3`) ported the op graph, `Simplify` and `Op`. Slice 4 (`port/pathops-4`)
completes the public surface and the op tests. Manifest entries flipped to `passing` in slice 4 (25):
`PathOpsOpTest.cpp`: `PathOpsOp`, `PathOpsFailOp`, `PathOpsRepOp`, `bug_513820666`;
`PathOpsBuilderTest.cpp`: all 11; `PathOpsTightBoundsTest.cpp`: all 9; `PathOpsAsWindingTest.cpp::PathOpsAsWinding`.

## Ported (crates/skia-rust-pathops/src)

| File | Skia source (m156) | State |
|---|---|---|
| `op_state.rs`, `op_span.rs`, `op_segment.rs`, `op_contour.rs`, `op_angle.rs`, `op_curve.rs` | SkOpGlobalState, SkOpSpan, SkOpSegment, SkOpContour, SkOpAngle, SkPathOpsCurve | complete |
| `op_coincidence.rs`, `op_winding.rs`, `path_writer.rs`, `op_edge_builder.rs`, `op_add_intersections.rs`, `op_common.rs` | SkOpCoincidence, SkPathOpsWinding, SkPathWriter, SkOpEdgeBuilder, SkAddIntersections, SkPathOpsCommon | complete |
| `op_simplify.rs` | SkPathOpsSimplify.cpp | complete (`pub fn simplify`) |
| `op_op.rs` | SkPathOpsOp.cpp | complete (`pub fn op`) |
| `op_builder.rs` | SkOpBuilder.cpp, `include/pathops/SkPathOps.h` (SkOpBuilder) | complete (`OpBuilder`) |
| `tight_bounds.rs` | SkPathOpsTightBounds.cpp | complete (`pub fn tight_bounds`) |
| `as_winding.rs` | SkPathOpsAsWinding.cpp | complete (`pub fn as_winding`) |
| `path_ops_ext.rs` | (no Skia counterpart) | `PathOpsExt` trait on `Path`: `op`, `simplify`, `tight_bounds`, `as_winding`; re-exported by the `skia-rust` facade |

Public surface: `skia_rust_pathops::{op, simplify, tight_bounds, as_winding, OpBuilder, PathOpsExt}`
and `path_op::PathOp`. The facade re-exports `skia_rust::{pathops, PathOpsExt}`. Row added to
`docs/API_MAPPING.md`.

## Tests (tests/src/unit)

| Test file | Ported | Passing |
|---|---|---|
| PathOpsSimplifyTest.cpp | 2 DEF_TESTs | 2/2 |
| PathOpsOpTest.cpp (`path_ops_op_test.rs`) | all 446 static functions, `tests[]` (all entries), `failTests[]`, `repTests[]`, `bug_513820666`, `PathOpsOp`, `PathOpsFailOp`, `PathOpsRepOp` | 4 DEF_TESTs, all `PathOpsOp` cases pass (run as one DEF_TEST) |
| PathOpsBuilderTest.cpp (`path_ops_builder_test.rs`) | 11 | 11/11 |
| PathOpsTightBoundsTest.cpp (`path_ops_tight_bounds_test.rs`) | 9 (threaded runners run single-threaded, same cases) | 9/9 |
| PathOpsAsWindingTest.cpp (`path_ops_as_winding_test.rs`) | 1 (with bug12040_1..5, bug13496_1..3) | 1/1 |
| (new) `path_ops_ext_test.rs` | `PathOpsExt` forwards to free functions (not a Skia test) | 1/1 |

Helpers: `tests/src/unit/path_ops_extended_test.rs` has `inner_simplify`, `test_simplify`,
`test_simplify_fail`, `compare_paths`, `test_path_op`, `test_path_op_check`, `test_path_op_fuzz`,
`test_path_op_fail`. `tests/src/unit/path_ops_test_common.rs` has `cubic_path_to_quads` (with the
`CubicToQuads` helpers).

Deviations, to report with the PR:

1. `innerPathOp` only compares the result with the region boundary when `reporter->verbose()`.
   That comparison (`comparePaths` with `SkRegion::op` and `getBoundaryPath`) is not ported. Our
   `test_path_op` checks success only, which is exactly what Skia checks without `--verbose`. So the
   op results are not geometrically verified by these tests.
2. `testSimplify` in our port compares the result on every run (Skia compares only when verbose).
3. `comparePaths` uses Skia's bitmap comparison (`pathsDrawTheSame`, `MAX_ERRORS` 9).
4. `Simplify` no longer calls `set_phase(Walking)`: that call is `DEBUG_VALIDATE` only in Skia.
5. `RunTestSet` is ported for the null-argument path only (`PathOpsOp`, `PathOpsFailOp`, `PathOpsRepOp`
   call it that way); `subTests[]` and reverse order are gated by `runSubTests`/`runReverse`, false in Skia.
6. `cubicOp130a` calls `complex_to_quads(pts2, &path)` where Skia's source also writes into `path`
   (not `pathB`). Kept as in Skia.
7. `AsWinding`: Skia's `cubic` branch of `left_edge` reads `t`, which is only set by the quad and
   conic branches (`SK_INIT_TO_AVOID_WARNING` = 0). Ported as `t = 0`.

## Not done in slice 4

Item 4 of the slice plan: other PathOps test files are still `todo`. Not started:
`PathOpsSimplifyFailTest`, `PathOpsSimplifyQuadThreadedTest`, `PathOpsSimplifyRectThreadedTest`,
`PathOpsSimplifyTrianglesThreadedTest`, `PathOpsSimplifyDegenerateThreadedTest`,
`PathOpsSimplifyQuadralateralsThreadedTest`, `PathOpsIssue3651`, `PathOpsInverseTest`,
`PathOpsChalkboardTest`, `PathOpsFuzz763Test`, `PathOpsSkpTest`, `PathOpsBattles`,
`PathOpsBuildUseTest`, `PathOpsBuilderConicTest`, `PathOpsOp*ThreadedTest`, `PathOpsThreeWayTest`,
`PathOpsTigerTest`.

## Next steps

1. Port the remaining test files above (PathOpsSimplifyFail and the threaded Simplify runners first:
   small and based on helpers that exist). Fuzz763, Issue3651 and Skp are large data files; the
   translator used for PathOpsOpTest (generate Rust from the C++ statements) applies to them.
2. Run `cargo xtask inventory verify` in full (it runs the whole test suite, longer than one
   foreground call; split by test filter if needed) and confirm the 25 flipped entries.
3. Consider the verbose region comparison (deviation 1) as a separate check to catch wrong geometry.

## Slice 5 (`port/pathops-5`): remaining test files

Ported and passing (101 of 101 ported unit entries; `cargo test -p skia-rust-tests -- path_ops`
runs them in about 100 s in debug): `PathOpsSimplifyFail` (+ `DontFailOne`), the five threaded
Simplify runners (`Quads`, `Rects`, `Triangles`, `Degenerates`, `Quadralaterals`), `OpCircle`,
`OpCubic`, `OpLoop`, `OpRect` (`Rects`, `Fast`), `QuadLineIntersectionThreaded`, `Chalkboard`,
`Fuzz763`, `Battle` (`battles`), `BuildUse`, `BuilderConic` (6), `Inverse`, `Issue3651`, `Skp`,
`ThreeWay` (2), `Tiger`. Each threaded runner is run single-threaded with the same enumeration
(`allowExtendedTest()` is false, so the runners stop after their first outer iteration, as in Skia).

Deviations, to keep in mind:

4. `SkIntersections::insert` overflow: Skia's `SkOPASSERT(0)` is skipped for the fuzz cases
   (`SkipAssert::kYes`). The port has no `debugSkipAssert` flag, so the `debug_assert!` was removed
   (the release behavior is kept). `PathOpsSimplifyFail` needs this.
5. The threaded runners' verbose output (the generated test sources, `outputProgress`) and the
   verbose-only `comparePaths` in `testSimplify` are not ported (they run only with `--verbose`).
6. `PathOpsChalkboard` and `PathOpsTiger` keep Skia's truncation: `PathOpsThreadState` stores
   `fA`/`fB` as `unsigned char`, so each runnable carries only the low byte of each 32-bit half of
   `testlines`.
7. `PathOpsBuilderConic`'s `setupOne` evaluates the arguments of `setXYWH` left to right (the C++
   order is unspecified); the results of `comparePaths` are ignored by that test, as in Skia.
8. `SixtyOvals` reuses one `trialRuns` counter across the outer loops (as Skia does), so only the
   first (col, row, rot) combination runs its 100 trials.

Still todo (reasons in the manifest):
- `PathOpsAngleTest.cpp`: `FindCrossEpsilon`, `FindQuadEpsilon`, `FindSlop` are disabled by
  `gDisableAngleTests = true` in Skia; `Circle`, `After`, `AllOnOneSide` need the SkOpAngle /
  SkOpContour test hooks (`debugAddAngle`, `debugLastAngle`, `PathOpsAngleTester::Orderable`).
- `PathOpsAngleIdeas.cpp`: four entries are gated by `gPathOpsAngleIdeasVerbose = false`;
  `OverlapHullsOne` needs `testQuadAngles`.
- GMs (`pathopsinverse` x2, `pathreverse`), bench (8) and fuzz (2) entries are not unit tests and
  are left for their own slices.

Manifest hygiene: 12 entries marked `passing` pointed at test modules whose file names did not
match the xtask mapping (`PathOpsDCubicTest` -> `path_ops_d_cubic_test`, etc.); the files were
renamed to match, and `PathOpsIssue3651` is `path_ops_issue3651`.

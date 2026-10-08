# PathOps op graph (slice 3): status and hand-over

Branch `port/pathops-3` (based on the unmerged `port/pathops-2`, a2cab7d).
Manifest entries flipped to `passing`: **0**. No `PathOps*` test is ported yet, so no
`tests/src/unit/path_ops_*` file was added. `inventory/manifest.toml` is untouched.

## What is in the tree (not yet in `lib.rs`)

The new modules are in `crates/skia-rust-pathops/src/` but are **not declared in `lib.rs`**, so
the crate still builds as before. To wire them in, add `pub mod` lines for each file below.

| File | Ported from (m156) | State |
|---|---|---|
| `op_state.rs` | `SkPathOpsTypes.h` (`SkOpGlobalState`, `SkOpPhase`), `SkOpCoincidence.h` fields | arenas, ids (`ContourId`, `SegId`, `SpanId`, `PtTId`, `AngleId`, `CoinId`), global flags. Complete. |
| `op_span.rs` | `SkOpSpan.h`, `SkOpSpan.cpp` | every `SkOpPtT`/`SkOpSpanBase`/`SkOpSpan` method. Complete except the calls into coincidence and winding (see below). |
| `op_curve.rs` | `SkPathOpsCurve.h/.cpp` (dispatch tables, `SkDCurve`, `SkDCurveSweep`, `nearPoint`, bounds) | complete. `DCurveBuf` is `SkDCurve` as points + conic weight. |
| `op_segment.rs` | `SkOpSegment.h/.cpp` | every method except the ones that live in `SkPathOpsWinding.cpp` (`sortableTop`, `rayCheck`, `windingSpanAtT`) and `addCurveTo`'s writer (see `PathWriter` below). Complete in draft; not yet type-checked against borrowck. |
| `op_contour.rs` | `SkOpContour.h/.cpp` | complete except `sortableTop`/`findSortableTop`/`rayCheck` (winding file). `ContourBuilder` is `SkOpContourBuilder`. |
| `op_angle.rs` | `SkOpAngle.h/.cpp` | every method incl. `after`, `orderable`, `endsIntersect`, `setSector`, `computeSector`, `insert`/`merge`/`loopCount`/`loopContains`/`previous`. Draft. |
| `path_op.rs` | `include/pathops/SkPathOps.h` (`SkPathOp`) | complete. |

Compile status (checked with the modules temporarily wired into `lib.rs`): **10 errors, all
missing names**, and 4 unused-variable warnings:

- `coin_contains`, `coin_extend`, `coin_add`, `coin_release_seg`, `coin_mark_collapsed`,
  `coin_release_deleted`, `coin_fix_up`: `SkOpCoincidence` (not ported yet).
- `span_sortable_top`: `SkPathOpsWinding.cpp` (not ported yet).
- `path_writer` module: `SkPathWriter` (not ported yet). The calls use points:
  `deferred_move(Point)`, `deferred_line(Point) -> bool`, `quad_to(Point, Point)`,
  `conic_to(Point, Point, f32)`, `cubic_to(Point, Point, Point)`. Check against `SkPathWriter.h`.
- Warnings: unused `seg` in `seg_fixup`-style helpers, `weight` in `seg_add_curve_to`'s
  caller, `this_end`/`rh_end` in `angle_set_sector`. Fix when wiring in.

## Design (follow this when continuing)

- One `OpState` (SkOpGlobalState) owns every arena: `contours`, `segments`, `spans`, `ptts`,
  `angles`, `coin_spans`. Contour 0 is the head contour. Nothing is freed during an operation,
  like Skia's arena.
- A `Span` holds the fields of `SkOpSpanBase` and `SkOpSpan` together. The `SkOpPtT` embedded in a
  span is a `PtT` in `ptts`, whose id is `Span::ptt`. Rings (`PtT::next`, `Span::coin_end`,
  `Span::coincident`, angle `next`) use self-ids for "points to itself".
- Graph methods are `impl OpState` blocks, named after the C++ owner (`span_*`, `ptt_*`, `seg_*`,
  `contour_*`, `angle_*`, `coin_*`), spread over the files above.
- Curves: `Segment::pts` is a copy of the points. Skia stores a pointer into the edge builder's
  array. Confirm nothing writes through that pointer (the edge builder) before relying on the copy.
- `SkPoint` distances (`SkPointPriv::DistanceToSqd`) are `f32`, because Skia returns `SkScalar`.
  Double-precision distances stay `f64`.
- `SK_NaN32` is used as a sentinel in `seg_compute_sum`. The value is set to `i32::MIN + 1`
  (`op_segment.rs`). **Verify against `SkFloatingPoint`/`SkTypes`** before relying on it.
- `kActiveEdge` (`op_segment.rs`) is transcribed literally from `SkOpSegment.cpp#L26-L44`.

## Not started

1. `SkOpCoincidence.cpp` (1437 lines). `SkCoincidentSpans` methods were read. `SkOpCoincidence`
   `extend`, `add`, `addEndMovedSpans` (both forms), `addExpanded`/`apply` and `addOrOverlap`,
   `checkOverlap`, `addIfMissing`, `TRange` were read in part. Still to read: `addOverlap`,
   `overlap`, `findOverlaps`, `addMissing`, `correctEnds`, `expand`, `contains`, `mark`,
   `markCollapsed`, `release`, `releaseDeleted`, `fixUp`, `restoreHead`, `Ordered`.
2. `SkPathOpsWinding.cpp` (443 lines): `sortableTop`, `findSortableTop`, `rayCheck`,
   `windingSpanAtT`, `SkOpRayHit` and the ray-cast helpers.
3. `SkOpEdgeBuilder` (363), `SkAddIntersections` (595), `SkPathOpsCommon` (338),
   `SkPathWriter` (453), `SkOpBuilder` (211), `SkPathOpsOp` (391), `SkPathOpsSimplify` (285),
   `SkPathOpsTightBounds` (83), `SkPathOpsAsWinding` (460).
4. Public API (`skia-rust-pathops`): `op`, `simplify`, `tight_bounds`, `as_winding`, `OpBuilder`.
   **Decision needed:** `Path::op/simplify/tight_bounds/as_winding` cannot be inherent methods in
   `skia-rust-core`, because `pathops` depends on `core` and not the other way round. Options: an
   extension trait in the `skia-rust` facade crate, or a `pathops` extension trait re-exported
   there. The facade route matches skia-safe's method names best.
5. Tests: `PathOpsSimplifyTest` (10k lines, lines first, then quads/cubics, then Op),
   `PathOpsTightBoundsTest`, `PathOpsAsWindingTest`, `PathOpsBuilderTest`, `PathOpsOpTest`
   (12.5k lines), and the threaded runners (single-threaded). `PathOpsExtendedTest` helpers
   (`testPathOp`, `testSimplify`, `comparePaths`) must be ported first.

## Systematic failures

None observed yet, because nothing runs. Expect the first divergences in the coincidence code
(`addExpanded`, `expand`, `checkOverlap` order of checks) and in the angle sort (`after`).
When a test fails, find the first divergence against `SkOpSegment.cpp`/`SkOpAngle.cpp` before
changing code.

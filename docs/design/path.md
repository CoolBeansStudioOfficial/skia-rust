# Path data model (SkPath family, chrome/m156)

m156 restructured Skia's paths: `SkPathRef` is gone, `SkPath` is immutable, and all editing
happens in `SkPathBuilder`. skia-rust follows the m156 C++ for the internals and `skia-safe` for
the public API shape where it still applies.

## Types and ownership

| Skia | skia-rust | Owns |
|---|---|---|
| `SkPathData` (`sk_sp`, immutable, refcounted) | `path_data::PathData`, shared as `Arc<PathData>` | points, conic weights, verbs (`Box<[_]>`), bounds, segment mask, unique id, convexity cache, the "is-a" tag (oval / rrect + direction + start index) |
| `SkPathRaw` (non-owning view) | `path_raw::PathRaw<'a>` (`Copy`) | nothing: borrows the three slices, plus bounds, fill type, convexity, segment mask |
| `SkPathRawShapes::{Rect,Oval,RRect,Triangle}` (stack storage + `SkPathRaw` base) | `path_raw_shapes::{Rect,Oval,RRect,Triangle}` | the points in a fixed array; `raw()` returns the `PathRaw` view (C++ inheritance becomes a method) |
| `SkPath` | `path::Path { data: Arc<PathData>, fill_type, is_volatile }` | a shared reference; `Clone` is an `Arc` clone (C++ copy) |
| `SkPathBuilder` | `path_builder::PathBuilder` | `Vec`s of points / verbs / weights, fill type, volatility, convexity, segment mask, last-move index, is-a tag |

* `sk_sp<SkPathData>` that may be null becomes `Option<Arc<PathData>>`. The C++ factories that return
  `nullptr` for invalid or non-finite input return `None`.
* **Copy-on-write.** `PathData` is never mutated after it is shared, exactly as in C++ (m156 has no
  COW: a builder copies the data in `addRaw`, and `snapshot()` makes a new `PathData`). The only
  post-construction writes are the ones C++ also makes before publishing an object (`setupIsA`,
  `setConvexity` right after `MakeTransform`); Rust does them through `Arc::get_mut` on the
  fresh `Arc`, so a shared `PathData` can never change shape.
* **Singletons.** `SkPathData::Empty()` and `SkPath::PeekErrorSingleton()` are two distinct
  process-wide `Arc<PathData>`s held in `OnceLock`s. `Path::is_finite()` is
  `!Arc::ptr_eq(data, error_singleton)`, mirroring the C++ pointer comparison.

## Caches

* **Unique / generation id.** Each `PathData` takes the next value of a global relaxed
  `AtomicU64` (starting at 1) when it is created, so `Path::generation_id()` changes exactly when
  C++'s does (a fill-type change shares the data and keeps the id).
* **Convexity.** `PathData::convexity` is an `AtomicU8` (relaxed), lazily resolved through
  `&self` (`get_resolved_convexity`) as the C++ `mutable std::atomic<uint8_t>`. Transforms
  propagate it with `path_priv::transform_convexity`. `Path::set_convexity` (used by
  `SkPathPriv::SetConvexity` / `CreateDrawArcPath`) writes the shared atomic, as in C++.
* **First direction.** m156 has no separate first-direction cache: it is derived from a convex
  convexity value, otherwise recomputed (`path_priv::compute_first_direction`).
* `SkIDChangeListener` (gen-id listeners) is not ported.

## Iteration

* `path_iter::PathIter` (m156 `SkPathIter`) yields `PathIterRec` values that carry their points by
  value (`points()` returns a slice of at most 4); C++ returns spans into the path or into the
  iterator's close storage, which Rust cannot lend from an `Iterator`.
* `path_priv::Iterate` (`SkPathPriv::Iterate` / `SkPath::RangeIter`) yields
  `(PathVerb, &[Point], Option<scalar>)`; the slice starts one point before the verb's own points
  (the "backset"), exactly like the C++ pointer arithmetic, so `pts[0]` is the current point.
* `path::Iter` (`SkPath::Iter`, auto-close + force-close) and `path::RawIter` keep their C++ state
  machines and are Rust `Iterator`s over `IterRec`.
* `path_priv::PathEdgeIter` (`SkPathEdgeIter`) keeps its index-based state; `next()` returns an
  `EdgeResult` with a copy of the edge's points.

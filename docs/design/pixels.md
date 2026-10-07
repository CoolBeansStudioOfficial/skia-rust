# Design: pixel access (`PixelRef`, `Bitmap`, `Pixmap`)

Status: implemented with D8 (2026-10-07). Closes issue #22. Pin: `chrome/m156`.

## Problem

`SkPixelRef` owns (or points at) pixel memory that any number of `SkBitmap`s share, and every
`SkPixmap` taken from them aliases it through raw pointers; `SkBitmap::eraseColor` and friends
write through `const` methods. PR #19 modelled the pixels as `RwLock<Vec<u8>>` and let a
`Pixmap<'a>` borrowed from a bitmap hold the lock guard. Any other pixel-touching call on the same
thread while that pixmap lived (`bm.peek_pixels()` then `bm.erase_color(..)`) blocked forever.

## Decision

No locks. Exclusivity is the borrow checker's job, sharing is `Arc`, and a write to shared pixels
copies them first (copy-on-write):

| | Read | Write |
|---|---|---|
| `PixelRef` | `pixels(&self) -> &[u8]` | `pixels_mut(&mut self) -> &mut [u8]` (detaches if shared) |
| `Bitmap` | `pixmap(&self)`, `peek_pixels(&self) -> Option<Pixmap<'_>>`, `get_color`, `read_pixels`, … | `peek_pixels_mut(&mut self) -> Option<Pixmap<'_>>`, `erase*`, `set_addr*`, `write_pixels` (all `&mut self`) |
| `Pixmap<'a>` | borrows `&'a [u8]` | borrows `&'a mut [u8]` |

- **`PixelRef`** is a cheap handle around `Arc<Inner>`; `Inner` holds the bytes as a plain
  `Vec<u8>` (or shared immutable `Data`). `pixels()` hands out `&[u8]` for as long as the handle is
  borrowed. `pixels_mut()` uses `Arc::get_mut`: when this handle is the only one (and the bytes are
  owned, not `Data`) it returns the bytes in place; otherwise it first replaces the handle's `Arc`
  with a fresh `Inner` holding a copy of the bytes (same dimensions and row bytes, mutable, no
  listeners, no release proc, a new generation ID). Like `Arc::make_mut`.
- **`Bitmap`** keeps `Option<PixelRef>` plus the byte offset of its first pixel. Cloning a bitmap
  shares the pixel ref (as `SkBitmap`'s copy constructor does). Methods that write pixels take
  `&mut self` and go through `PixelRef::pixels_mut`, so a write never affects another bitmap.
- **`Pixmap<'a>`** is `ImageInfo` + row bytes + `Option<&'a [u8]>` or `&'a mut [u8]` (the
  `Shared`/`Unique` storage of PR #19 minus the two guard variants). `Pixmap::new` /
  `new_readonly` wrap caller memory exactly as before.

Why this cannot deadlock or panic: there is no lock or `RefCell` anywhere on the pixel path. A
`Pixmap` borrowed from `&bm` coexists with any other `&bm` call; one borrowed from `&mut bm` makes
every other use of `bm` a compile error until it is dropped.

## Semantics compared to Skia

Identical whenever a bitmap's pixel ref is not shared with another live handle, which is every
bitmap a test or a canvas allocates and draws into. Generation IDs are then exactly Skia's: a write
through `erase*`/`write_pixels` calls `notify_pixels_changed` (new ID), a raw write through
`peek_pixels_mut`/`set_addr*` does not.

Different, by construction, when the pixel ref *is* shared (a clone, `extract_subset`, a
`pixel_ref()` handle someone kept):

- Skia writes through the shared memory, so every sharer sees the new pixels and the new
  generation ID. Here the writer detaches onto a copy: other sharers keep the old pixels, the old
  pixel ref, its generation ID and its listeners; the writer's pixel ref is new, so its generation
  ID is new (even for a raw `peek_pixels_mut` write, which in Skia keeps the ID).
- A detached copy is mutable even if the shared pixel ref was immutable (Skia only debug-asserts
  writes to immutable pixels).

The one Skia pattern this rules out is "draw into one handle, read through another": `SkCanvas
canvas(bitmap)` keeps a *copy* of `bitmap` and draws into the shared pixels. In skia-rust a canvas
or surface that draws into caller pixels borrows them mutably (as skia-safe's
`surfaces::wrap_pixels(&mut [u8], …) -> Borrows<'_, Surface>` does) and the caller reads the
results after the borrow ends. The GM harness's surface stub already works that way
(`Surface::wrap_pixels(&mut Bitmap, …)` moves the bitmap in and puts it back on drop); D6 builds
`Canvas`/`Surface` on this rule.

## Alternatives considered

- **Keep `RwLock`, take it per call.** A `Pixmap` must expose `&[u8]`, so it has to hold a guard;
  the deadlock is inherent.
- **A re-entrancy-checking lock / `RefCell`-style cell that panics instead of blocking.** Keeps
  Skia's aliasing, but turns the deadlock into a run-time panic, still needs a guard inside every
  `Pixmap`, and contends across threads.
- **Atomic bytes (`[AtomicU8]`).** No `&[u8]` to give the raster pipeline, blitters or users.

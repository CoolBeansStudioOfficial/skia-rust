// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkArenaAlloc.h (the role, not the implementation)

//! `SkArenaAlloc`: objects that live as long as the arena, handed out by shared reference.
//!
//! Skia's arena is a bump allocator over raw memory with a destructor list. Here it is a safe
//! arena built on [`OnceCell`] slots in chunks that never move: [`TypedArena`] holds values of
//! one type, and [`ArenaAlloc`] keeps one `TypedArena` per type it has seen. `alloc`/`make`
//! take `&self` and return `&T` borrowed from the arena, which is what lets a
//! [`RasterPipeline<'a>`](crate::raster_pipeline::RasterPipeline) borrow the contexts its
//! appenders allocate (`appendMatrix(alloc, …)` and friends). Values are dropped with the arena,
//! or by [`reset`](ArenaAlloc::reset), which keeps the chunks so that a reused arena does not
//! allocate again (`SkArenaAllocWithReset`).

use core::any::Any;
use core::cell::{Cell, OnceCell};
use core::fmt;

/// Slots in a [`TypedArena`]'s first chunk; each further chunk doubles.
const FIRST_CHUNK: usize = 8;

/// A chunk of slots; never moves once allocated (it is boxed).
struct Chunk<T> {
    slots: Box<[OnceCell<T>]>,
    next: OnceCell<Box<Chunk<T>>>,
}

impl<T> Chunk<T> {
    fn new(len: usize) -> Chunk<T> {
        Chunk {
            slots: (0..len).map(|_| OnceCell::new()).collect(),
            next: OnceCell::new(),
        }
    }
}

/// An arena of `T`s: [`alloc`](Self::alloc) moves a value in and returns a reference that lives
/// as long as the arena.
///
/// Works for any `T`, including types that borrow (`TypedArena<GatherCtx<'p>>`).
pub struct TypedArena<T> {
    first: OnceCell<Box<Chunk<T>>>,
    len: Cell<usize>,
}

impl<T> Default for TypedArena<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> fmt::Debug for TypedArena<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TypedArena")
            .field("len", &self.len.get())
            .finish_non_exhaustive()
    }
}

impl<T> TypedArena<T> {
    /// An empty arena (allocates nothing until the first [`alloc`](Self::alloc)).
    #[must_use]
    pub const fn new() -> TypedArena<T> {
        TypedArena {
            first: OnceCell::new(),
            len: Cell::new(0),
        }
    }

    /// Moves `value` into the arena and returns a reference to it.
    ///
    /// # Panics
    /// Never: the slot at index `len` is always empty (the checks guard that invariant).
    pub fn alloc(&self, value: T) -> &T {
        let mut i = self.len.get();
        self.len.set(i + 1);
        let mut chunk: &Chunk<T> = self.first.get_or_init(|| Box::new(Chunk::new(FIRST_CHUNK)));
        loop {
            if let Some(slot) = chunk.slots.get(i) {
                // Slots at and past `len` are empty: `len` only grows through `&self`, and
                // `reset` empties every slot.
                assert!(slot.set(value).is_ok(), "TypedArena: slot in use");
                return slot.get().expect("TypedArena: slot just set");
            }
            i -= chunk.slots.len();
            let next_len = chunk.slots.len() * 2;
            chunk = chunk.next.get_or_init(|| Box::new(Chunk::new(next_len)));
        }
    }

    /// The number of values in the arena.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len.get()
    }

    /// Whether the arena holds no values.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len.get() == 0
    }

    /// The values, in allocation order.
    pub fn iter(&self) -> impl Iterator<Item = &T> + '_ {
        let mut chunk = self.first.get().map(|c| &**c);
        let mut index = 0;
        core::iter::from_fn(move || {
            loop {
                let c = chunk?;
                if let Some(slot) = c.slots.get(index) {
                    index += 1;
                    return slot.get();
                }
                chunk = c.next.get().map(|n| &**n);
                index = 0;
            }
        })
        .take(self.len.get())
    }

    /// Drops every value, keeping the chunks for reuse.
    pub fn reset(&mut self) {
        let mut chunk = self.first.get_mut().map(|c| &mut **c);
        while let Some(c) = chunk {
            for slot in &mut c.slots {
                drop(slot.take());
            }
            chunk = c.next.get_mut().map(|n| &mut **n);
        }
        self.len.set(0);
    }
}

/// The type-erased interface [`ArenaAlloc`] keeps its typed arenas behind.
trait ErasedArena {
    fn as_any(&self) -> &dyn Any;
    fn reset(&mut self);
    fn is_empty(&self) -> bool;
}

impl<T: 'static> ErasedArena for TypedArena<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn reset(&mut self) {
        TypedArena::reset(self);
    }

    fn is_empty(&self) -> bool {
        TypedArena::is_empty(self)
    }
}

/// `SkArenaAlloc`: an arena for values of any `'static` type, handed out as `&T` that live as
/// long as the arena. (For types that borrow, use a [`TypedArena`].)
///
/// Values of one type are stored together in a [`TypedArena`]; [`reset`](Self::reset) drops
/// every value but keeps the storage (`SkArenaAllocWithReset`).
#[doc(alias = "SkArenaAlloc")]
#[doc(alias = "SkArenaAllocWithReset")]
#[derive(Default)]
pub struct ArenaAlloc {
    arenas: TypedArena<Box<dyn ErasedArena>>,
}

impl fmt::Debug for ArenaAlloc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArenaAlloc")
            .field("types", &self.arenas.len())
            .finish_non_exhaustive()
    }
}

impl ArenaAlloc {
    /// An empty arena (allocates nothing until the first [`make`](Self::make)).
    #[must_use]
    pub const fn new() -> ArenaAlloc {
        ArenaAlloc {
            arenas: TypedArena::new(),
        }
    }

    /// The arena holding `T`s, created on first use.
    fn typed<T: 'static>(&self) -> &TypedArena<T> {
        if let Some(arena) = self
            .arenas
            .iter()
            .find_map(|a| a.as_any().downcast_ref::<TypedArena<T>>())
        {
            return arena;
        }
        self.arenas
            .alloc(Box::new(TypedArena::<T>::new()))
            .as_any()
            .downcast_ref::<TypedArena<T>>()
            .expect("ArenaAlloc: arena of the requested type")
    }

    /// `make<T>(args…)`: moves `value` into the arena and returns a reference to it.
    pub fn make<T: 'static>(&self, value: T) -> &T {
        self.typed::<T>().alloc(value)
    }

    /// `isEmpty()`: whether nothing has been allocated since creation or the last
    /// [`reset`](Self::reset).
    #[doc(alias = "isEmpty")]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.arenas.iter().all(|a| a.is_empty())
    }

    /// `SkArenaAllocWithReset::reset()`: drops every value, keeping the storage for reuse.
    pub fn reset(&mut self) {
        let mut chunk = self.arenas.first.get_mut().map(|c| &mut **c);
        while let Some(c) = chunk {
            for slot in &mut c.slots {
                if let Some(arena) = slot.get_mut() {
                    arena.reset();
                }
            }
            chunk = c.next.get_mut().map(|n| &mut **n);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ArenaAlloc, TypedArena};
    use core::cell::Cell;

    #[test]
    fn typed_arena_keeps_values_in_place() {
        let arena = TypedArena::new();
        let refs: Vec<&u32> = (0..100).map(|i| arena.alloc(i)).collect();
        for (i, r) in refs.iter().enumerate() {
            assert_eq!(**r as usize, i);
        }
        assert_eq!(arena.len(), 100);
        assert_eq!(
            arena.iter().copied().collect::<Vec<_>>(),
            (0..100).collect::<Vec<_>>()
        );
    }

    #[test]
    fn typed_arena_holds_borrowing_values() {
        let data = [1u8, 2, 3];
        let arena: TypedArena<&[u8]> = TypedArena::new();
        let r = arena.alloc(&data[1..]);
        assert_eq!(*r, &[2, 3]);
    }

    #[test]
    fn arena_alloc_mixes_types_and_resets() {
        struct Dropper<'c>(&'c Cell<u32>);
        impl Drop for Dropper<'_> {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }

        let mut alloc = ArenaAlloc::new();
        assert!(alloc.is_empty());
        let a = alloc.make([1.0f32, 2.0]);
        let b = alloc.make(7u16);
        let c = alloc.make([3.0f32, 4.0]);
        assert_eq!((*a, *b, *c), ([1.0, 2.0], 7, [3.0, 4.0]));
        assert!(!alloc.is_empty());
        alloc.reset();
        assert!(alloc.is_empty());
        assert_eq!(*alloc.make(9u16), 9);

        let drops = Cell::new(0);
        {
            let arena = TypedArena::new();
            for _ in 0..20 {
                arena.alloc(Dropper(&drops));
            }
        }
        assert_eq!(drops.get(), 20);
    }
}

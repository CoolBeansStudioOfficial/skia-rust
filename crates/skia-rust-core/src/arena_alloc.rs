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
use core::cell::{Cell, OnceCell, RefCell};
use core::fmt;

use skia_rust_simd::rp::MemPtr;
use skia_rust_sksl::codegen::rp::SlotAlloc;

use crate::effect_priv::SHADER_SCRATCH;

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
    scratch_bytes: Cell<usize>,
    /// The initial contents of the scratch memory, from its start (the rest starts as zero).
    scratch_init: RefCell<Vec<u8>>,
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
            scratch_bytes: Cell::new(0),
            scratch_init: RefCell::new(Vec::new()),
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
    /// Reserves `bytes` of writable scratch memory for the pipeline built in this arena, and
    /// returns its byte offset in the scratch buffer.
    ///
    /// skia-rust: Skia's shaders allocate writable storage in the arena that the pipeline's
    /// stages read and write at run time (`rec.fAlloc->makeArray<float>(...)` for
    /// `store_src`/`load_dst`). Rust arenas hand out shared references, and writable pipeline
    /// memory is named by a [`MemSlot`](crate::raster_pipeline::MemSlot) bound per run, so the
    /// arena only counts the bytes: whoever runs the pipeline binds a buffer of
    /// [`scratch_bytes`](Self::scratch_bytes) bytes to
    /// [`SHADER_SCRATCH`](crate::effect_priv::SHADER_SCRATCH).
    ///
    /// # Panics
    /// If the scratch memory outgrows a `u32` offset.
    pub fn alloc_scratch(&self, bytes: usize) -> u32 {
        let offset = self.scratch_bytes.get();
        self.scratch_bytes.set(offset + bytes);
        u32::try_from(offset).expect("scratch memory offsets fit in a u32")
    }

    /// `makeBytesAlignedTo(bytes, align)` for scratch memory whose first `init.len()` bytes start
    /// as `init` (the rest start as zero): reserves `bytes` at an offset that is a multiple of
    /// `align` (relative to the start of the scratch buffer), and returns that offset.
    ///
    /// skia-rust: this is the initial-contents variant `SkSL` needs for its immutable slots, which
    /// Skia writes while it builds the stages. [`scratch_buffer`](Self::scratch_buffer) gives the
    /// bytes to bind.
    ///
    /// # Panics
    /// If `init` is longer than `bytes`, `align` is zero, or the scratch memory outgrows a `u32`
    /// offset.
    pub fn alloc_scratch_init(&self, bytes: usize, align: usize, init: &[u8]) -> u32 {
        assert!(
            init.len() <= bytes,
            "the initial contents exceed the allocation"
        );
        assert!(align > 0, "alignment must be positive");
        let offset = self.scratch_bytes.get().div_ceil(align) * align;
        self.scratch_bytes.set(offset + bytes);
        if !init.is_empty() {
            let end = offset + init.len();
            let mut image = self.scratch_init.borrow_mut();
            if image.len() < end {
                image.resize(end, 0);
            }
            image[offset..end].copy_from_slice(init);
        }
        u32::try_from(offset).expect("scratch memory offsets fit in a u32")
    }

    /// The bytes of scratch memory reserved with [`alloc_scratch`](Self::alloc_scratch) and
    /// [`alloc_scratch_init`](Self::alloc_scratch_init), zeroed except for their initial
    /// contents: what the code that binds [`SHADER_SCRATCH`](crate::effect_priv::SHADER_SCRATCH)
    /// passes to `MemView::write`.
    #[must_use]
    pub fn scratch_buffer(&self) -> Vec<u8> {
        let mut buffer = vec![0_u8; self.scratch_bytes.get()];
        let image = self.scratch_init.borrow();
        buffer[..image.len()].copy_from_slice(&image);
        buffer
    }

    /// The bytes of scratch memory reserved with [`alloc_scratch`](Self::alloc_scratch).
    #[must_use]
    pub fn scratch_bytes(&self) -> usize {
        self.scratch_bytes.get()
    }

    #[doc(alias = "isEmpty")]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.arenas.iter().all(|a| a.is_empty())
    }

    /// `SkArenaAllocWithReset::reset()`: drops every value, keeping the storage for reuse.
    pub fn reset(&mut self) {
        self.scratch_bytes.set(0);
        self.scratch_init.get_mut().clear();
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

// Port of: src/sksl/codegen/SkSLRasterPipelineBuilder.cpp#L1697-L1819 (chrome/m156)
// (the arena calls `appendStages` makes; the trait is `skia_rust_sksl`'s, see its docs)
impl<'a> SlotAlloc<'a> for ArenaAlloc {
    fn make<T: Copy + 'static>(&'a self, v: T) -> &'a T {
        ArenaAlloc::make(self, v)
    }

    fn alloc_scratch_init(&'a self, bytes: usize, align: usize, init: &[u8]) -> MemPtr {
        MemPtr::new(
            SHADER_SCRATCH,
            ArenaAlloc::alloc_scratch_init(self, bytes, align, init),
        )
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

    #[test]
    fn scratch_init_is_aligned_copied_and_reset() {
        let mut alloc = ArenaAlloc::new();
        // The first reservation starts at 0 and carries its initial contents.
        assert_eq!(alloc.alloc_scratch_init(8, 16, &[1, 2, 3]), 0);
        // The next one is aligned past the 8 bytes already reserved.
        assert_eq!(alloc.alloc_scratch_init(4, 16, &[9, 9, 9, 9]), 16);
        // A plain reservation adds zeros.
        assert_eq!(alloc.alloc_scratch(2), 20);
        assert_eq!(alloc.scratch_bytes(), 22);

        let buffer = alloc.scratch_buffer();
        assert_eq!(buffer.len(), 22);
        assert_eq!(&buffer[..3], &[1, 2, 3]);
        assert!(buffer[3..16].iter().all(|&b| b == 0));
        assert_eq!(&buffer[16..20], &[9, 9, 9, 9]);
        assert_eq!(&buffer[20..], &[0, 0]);

        alloc.reset();
        assert_eq!(alloc.scratch_bytes(), 0);
        assert_eq!(alloc.scratch_buffer().len(), 0);
    }
}

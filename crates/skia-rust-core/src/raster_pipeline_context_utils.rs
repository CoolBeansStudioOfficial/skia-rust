// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkRasterPipelineContextUtils.h

//! `SkRPCtxUtils`: packing small contexts into the stage's context slot.
//!
//! Skia's stage context is a `void*`; `Pack` stores a context no larger than a pointer in the
//! pointer bits themselves and copies larger ones into the arena. Rust stages hold typed
//! contexts (`rp::Stage`), so a packed context is a [`Packed`] value: the context itself, or a
//! reference to the arena's copy.

use crate::arena_alloc::ArenaAlloc;

/// A context packed by [`pack`]: inline when it fits in a pointer, otherwise in the arena.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Packed<'a, T> {
    /// The context's bits, held in place of the pointer (`sk_bit_cast<void*>(ctx)`).
    Inline(T),
    /// The arena's copy of the context (`alloc->make<T>(ctx)`).
    Allocated(&'a T),
}

/// Whether a `T` fits in a context pointer (`sizeof(T) <= sizeof(void*)`).
#[must_use]
pub const fn fits_in_pointer<T>() -> bool {
    size_of::<T>() <= size_of::<*const ()>()
}

// Port of: src/core/SkRasterPipelineContextUtils.h#L22-L35 (chrome/m156)
/// `SkRPCtxUtils::Pack`: checks if the context is small enough to fit directly in the context
/// field. If so, it is held inline; if not, a copy is allocated inside `alloc`.
#[doc(alias = "Pack")]
pub fn pack<'a, T: Copy + 'static>(ctx: &T, alloc: &'a ArenaAlloc) -> Packed<'a, T> {
    // If the context is small enough to fit in a pointer, bit-cast it; if not, alloc a copy.
    if fits_in_pointer::<T>() {
        Packed::Inline(*ctx)
    } else {
        Packed::Allocated(alloc.make(*ctx))
    }
}

// Port of: src/core/SkRasterPipelineContextUtils.h#L37-L51 (chrome/m156)
/// `SkRPCtxUtils::Unpack`: the reverse operation, either the inline bits or the arena's copy.
#[doc(alias = "Unpack")]
#[must_use]
pub fn unpack<T: Copy>(ctx: Packed<'_, T>) -> T {
    match ctx {
        Packed::Inline(v) => v,
        Packed::Allocated(r) => *r,
    }
}

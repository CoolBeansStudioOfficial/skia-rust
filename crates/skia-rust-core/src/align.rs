// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/private/SkAlign.h

//! Alignment helpers (`SkAlign.h`).
//!
//! C++ templates over the integer type become a trait implemented for every primitive
//! integer; arithmetic wraps as C++ unsigned arithmetic does.

/// Integer types accepted by the alignment helpers.
pub trait Align: Copy {
    /// `(x + 1) >> 1 << 1`
    #[must_use]
    fn align2(self) -> Self;
    /// `(x + 3) >> 2 << 2`
    #[must_use]
    fn align4(self) -> Self;
    /// `(x + 7) >> 3 << 3`
    #[must_use]
    fn align8(self) -> Self;
    /// `(x + 15) >> 4 << 4`
    #[must_use]
    fn align16(self) -> Self;
    /// `0 == (x & 1)`
    fn is_align2(self) -> bool;
    /// `0 == (x & 3)`
    fn is_align4(self) -> bool;
    /// `0 == (x & 7)`
    fn is_align8(self) -> bool;
    /// `0 == (x & 15)`
    fn is_align16(self) -> bool;
    /// Aligns up to a power of 2.
    #[must_use]
    fn align_to(self, alignment: Self) -> Self;
    /// Aligns up to a non power of 2.
    #[must_use]
    fn align_non_pow2(self, alignment: Self) -> Self;
}

macro_rules! impl_align {
    ($($t:ty),*) => {$(
        impl Align for $t {
            fn align2(self) -> Self { self.wrapping_add(1) >> 1 << 1 }
            fn align4(self) -> Self { self.wrapping_add(3) >> 2 << 2 }
            fn align8(self) -> Self { self.wrapping_add(7) >> 3 << 3 }
            fn align16(self) -> Self { self.wrapping_add(15) >> 4 << 4 }
            fn is_align2(self) -> bool { 0 == (self & 1) }
            fn is_align4(self) -> bool { 0 == (self & 3) }
            fn is_align8(self) -> bool { 0 == (self & 7) }
            fn is_align16(self) -> bool { 0 == (self & 15) }
            fn align_to(self, alignment: Self) -> Self {
                debug_assert!(alignment != 0 && (alignment & alignment.wrapping_sub(1)) == 0);
                self.wrapping_add(alignment).wrapping_sub(1) & !alignment.wrapping_sub(1)
            }
            fn align_non_pow2(self, alignment: Self) -> Self {
                (self.wrapping_add(alignment).wrapping_sub(1) / alignment).wrapping_mul(alignment)
            }
        }
    )*};
}
impl_align!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

// Port of: include/private/SkAlign.h#L15-L18 (chrome/m156)
/// Rounds up to a multiple of 2.
#[doc(alias = "SkAlign2")]
#[must_use]
pub fn align2<T: Align>(x: T) -> T {
    x.align2()
}
/// Rounds up to a multiple of 4.
#[doc(alias = "SkAlign4")]
#[must_use]
pub fn align4<T: Align>(x: T) -> T {
    x.align4()
}
/// Rounds up to a multiple of 8.
#[doc(alias = "SkAlign8")]
#[must_use]
pub fn align8<T: Align>(x: T) -> T {
    x.align8()
}
/// Rounds up to a multiple of 16.
#[doc(alias = "SkAlign16")]
#[must_use]
pub fn align16<T: Align>(x: T) -> T {
    x.align16()
}

// Port of: include/private/SkAlign.h#L20-L23 (chrome/m156)
/// True if `x` is a multiple of 2.
#[doc(alias = "SkIsAlign2")]
#[must_use]
pub fn is_align2<T: Align>(x: T) -> bool {
    x.is_align2()
}
/// True if `x` is a multiple of 4.
#[doc(alias = "SkIsAlign4")]
#[must_use]
pub fn is_align4<T: Align>(x: T) -> bool {
    x.is_align4()
}
/// True if `x` is a multiple of 8.
#[doc(alias = "SkIsAlign8")]
#[must_use]
pub fn is_align8<T: Align>(x: T) -> bool {
    x.is_align8()
}
/// True if `x` is a multiple of 16.
#[doc(alias = "SkIsAlign16")]
#[must_use]
pub fn is_align16<T: Align>(x: T) -> bool {
    x.is_align16()
}

/// Rounds up to a multiple of the pointer size.
// Port of: include/private/SkAlign.h#L26-L29 (chrome/m156)
#[doc(alias = "SkAlignPtr")]
#[must_use]
pub fn align_ptr<T: Align>(x: T) -> T {
    if size_of::<*const ()>() == 8 {
        x.align8()
    } else {
        x.align4()
    }
}

/// True if `x` is a multiple of the pointer size.
// Port of: include/private/SkAlign.h#L30-L33 (chrome/m156)
#[doc(alias = "SkIsAlignPtr")]
#[must_use]
pub fn is_align_ptr<T: Align>(x: T) -> bool {
    if size_of::<*const ()>() == 8 {
        x.is_align8()
    } else {
        x.is_align4()
    }
}

/// Aligns up to a power of 2.
// Port of: include/private/SkAlign.h#L38-L41 (chrome/m156)
#[doc(alias = "SkAlignTo")]
#[must_use]
pub fn align_to<T: Align>(x: T, alignment: T) -> T {
    x.align_to(alignment)
}

/// Aligns up to a non power of 2.
// Port of: include/private/SkAlign.h#L47-L49 (chrome/m156)
#[doc(alias = "SkAlignNonPow2")]
#[must_use]
pub fn align_non_pow2<T: Align>(x: T, alignment: T) -> T {
    x.align_non_pow2(alignment)
}

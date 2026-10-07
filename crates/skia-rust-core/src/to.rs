// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/private/SkTo.h

//! Checked integer narrowing (`SkTo`).

use crate::t_fits_in::t_fits_in;

/// Integer conversion with C++ `static_cast` semantics (two's complement wrap).
///
/// This is what `static_cast<D>(s)` does after `SkTo`'s assert; it is also used by
/// `SafeMath::cast_to`.
pub trait WrappingCast<D>: Copy {
    /// `static_cast<D>(self)`.
    fn wrapping_cast(self) -> D;
}

macro_rules! impl_wrapping_cast_from {
    ($s:ty => $($d:ty),*) => {$(
        impl WrappingCast<$d> for $s {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_possible_wrap)] // static_cast semantics
            fn wrapping_cast(self) -> $d {
                self as $d
            }
        }
    )*};
}
macro_rules! impl_wrapping_cast {
    ($($s:ty),*) => {$(
        impl_wrapping_cast_from!($s => i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);
    )*};
}
impl_wrapping_cast!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

/// Converts `s` to `D`, asserting (in debug builds) that it fits.
// Port of: include/private/SkTo.h#L16-L19 (chrome/m156)
#[doc(alias = "SkTo")]
#[must_use]
pub fn to<D, S>(s: S) -> D
where
    D: TryFrom<S>,
    S: WrappingCast<D>,
{
    debug_assert!(t_fits_in::<D, S>(s));
    s.wrapping_cast()
}

macro_rules! def_to {
    ($($alias:literal $name:ident => $t:ty;)*) => {$(
        #[doc = concat!("`", $alias, "`: [`to`] specialised to `", stringify!($t), "`.")]
        #[doc(alias = $alias)]
        #[must_use]
        pub fn $name<S>(x: S) -> $t
        where
            $t: TryFrom<S>,
            S: WrappingCast<$t>,
        {
            to::<$t, S>(x)
        }
    )*};
}

// Port of: include/private/SkTo.h#L21-L31 (chrome/m156)
def_to! {
    "SkToS8" to_s8 => i8;
    "SkToU8" to_u8 => u8;
    "SkToS16" to_s16 => i16;
    "SkToU16" to_u16 => u16;
    "SkToS32" to_s32 => i32;
    "SkToU32" to_u32 => u32;
    "SkToS64" to_s64 => i64;
    "SkToU64" to_u64 => u64;
    "SkToInt" to_int => i32;
    "SkToUInt" to_uint => u32;
    "SkToSizeT" to_size_t => usize;
}

/// Returns false or true based on the condition.
// Port of: include/private/SkTo.h#L35-L37 (chrome/m156)
#[doc(alias = "SkToBool")]
#[must_use]
pub fn to_bool<T: Default + PartialEq>(x: &T) -> bool {
    *x != T::default()
}

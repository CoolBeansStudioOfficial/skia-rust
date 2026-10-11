// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: fuzz/Fuzz.h, fuzz/Fuzz.cpp

//! The `Fuzz` byte reader: turns the fuzzer's input bytes into typed values, exactly as Skia's
//! does, so a given input drives a ported fuzzer through the same calls as the C++ one.
//!
//! | `Fuzz.h` | [`Fuzz`] |
//! |---|---|
//! | `next(T*)`, `next(a, b, …)` | [`Fuzz::next`], one call per out-parameter, left to right |
//! | `next(bool*)`, `nextBool()` | [`Fuzz::next_bool`] |
//! | `nextRange(T*, min, max)` (the template) | [`Fuzz::next_range`] |
//! | `nextRange(float*, float, float)` (the overload) | [`Fuzz::next_range_f32`] |
//! | `nextEnum(T*, max)` | [`Fuzz::next_enum`], on the enum's underlying integer |
//! | `nextN(T*, n)` | [`Fuzz::next_n`] |
//! | `next(SkRegion*)` | [`Fuzz::next_region`] |
//! | `signalBug()` | [`Fuzz::signal_bug`] (a panic) |
//!
//! Which `nextRange` C++ picks matters: `nextRange(&f, 0.0f, 100.0f)` is the float overload
//! (`fmod` into range), `nextRange(&f, -1000, 1000)` with `int` bounds is the template (a clamp
//! that lets NaN through). Port each call to the one C++ overload resolution chooses.
//!
//! Values are read with `memcpy`, so in host byte order; every platform skia-rust targets is
//! little-endian, and the committed corpora are written little-endian.

use skia_rust_core::region::Region;

/// A value `Fuzz::next` can read: a plain integer or float, read as its `size_of` bytes in host
/// order (`memcpy` in `Fuzz::nextBytes`).
pub trait FuzzValue: Copy + PartialOrd {
    /// `sizeof(T)`.
    const SIZE: usize;
    /// The value whose host-order bytes are `bytes` (`bytes.len() == SIZE`).
    fn from_ne_bytes(bytes: &[u8]) -> Self;
    /// `T(0)`, for [`Fuzz::next_enum`].
    fn zero() -> Self;
}

macro_rules! impl_fuzz_value {
    ($($t:ty => $zero:expr),* $(,)?) => {$(
        impl FuzzValue for $t {
            const SIZE: usize = size_of::<$t>();
            fn from_ne_bytes(bytes: &[u8]) -> Self {
                let mut raw = [0u8; size_of::<$t>()];
                raw.copy_from_slice(bytes);
                <$t>::from_ne_bytes(raw)
            }
            fn zero() -> Self {
                $zero
            }
        }
    )*};
}

impl_fuzz_value!(
    u8 => 0, i8 => 0, u16 => 0, i16 => 0, u32 => 0, i32 => 0, u64 => 0, i64 => 0,
    f32 => 0.0, f64 => 0.0,
);

/// Reads typed values from a fuzzer's input bytes (`class Fuzz`).
// Port of: fuzz/Fuzz.h#L23-L113 (chrome/m156)
#[derive(Debug)]
pub struct Fuzz<'a> {
    data: &'a [u8],
    next_byte: usize,
}

impl<'a> Fuzz<'a> {
    /// A reader over `data`.
    #[must_use]
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, next_byte: 0 }
    }

    /// The total number of "random" bytes available.
    #[must_use]
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// True if there are no bytes remaining for fuzzing.
    #[must_use]
    pub fn exhausted(&self) -> bool {
        self.data.len() == self.next_byte
    }

    /// Consumes every remaining byte.
    pub fn deplete(&mut self) {
        self.next_byte = self.data.len();
    }

    /// The number of bytes not yet read.
    #[doc(alias = "remainingSize")]
    #[must_use]
    pub fn remaining_size(&self) -> usize {
        self.data.len() - self.next_byte
    }

    /// The bytes not yet read.
    #[doc(alias = "remainingData")]
    #[must_use]
    pub fn remaining_data(&self) -> &'a [u8] {
        &self.data[self.next_byte..]
    }

    /// Fills `out` with the next bytes. Past the end, the rest of `out` is zero and the reader is
    /// exhausted.
    // Port of: fuzz/Fuzz.cpp#L19-L28 (chrome/m156)
    #[doc(alias = "nextBytes")]
    pub fn next_bytes(&mut self, out: &mut [u8]) {
        let size = out.len();
        if self.next_byte + size > self.data.len() {
            out.fill(0);
            let rest = &self.data[self.next_byte..];
            out[..rest.len()].copy_from_slice(rest);
            self.next_byte = self.data.len();
            return;
        }
        out.copy_from_slice(&self.data[self.next_byte..self.next_byte + size]);
        self.next_byte += size;
    }

    /// The next `T` (`next(T*)`).
    // Port of: fuzz/Fuzz.h#L66-L67 (chrome/m156)
    #[must_use]
    #[allow(clippy::should_implement_trait)] // `Fuzz::next`: generic over the value type and
    // never ends, so not an `Iterator`
    pub fn next<T: FuzzValue>(&mut self) -> T {
        let mut raw = [0u8; 8];
        let raw = &mut raw[..T::SIZE];
        self.next_bytes(raw);
        T::from_ne_bytes(raw)
    }

    /// The next `bool`: one byte, true when its low bit is set (`next(bool*)`).
    // Port of: fuzz/Fuzz.cpp#L12-L17 (chrome/m156)
    #[doc(alias = "nextBool")]
    #[must_use]
    pub fn next_bool(&mut self) -> bool {
        let n: u8 = self.next();
        (n & 1) == 1
    }

    /// The next `T`, clamped to `[min, max]` (the `nextRange` template). The bounds are already
    /// converted to `T`, as the template's `(Raw)min` / `(Raw)max` casts do; with a float `T`,
    /// NaN is not clamped.
    // Port of: fuzz/Fuzz.h#L121-L131 (chrome/m156)
    #[doc(alias = "nextRange")]
    #[must_use]
    pub fn next_range<T: FuzzValue>(&mut self, min: T, max: T) -> T {
        let mut raw: T = self.next();
        if raw < min {
            raw = min;
        }
        if raw > max {
            raw = max;
        }
        raw
    }

    /// The next float mapped into `[min, max + 1)` by `fmod` (the `nextRange(float*, float,
    /// float)` overload). Non-normal values other than zero become `max` first.
    // Port of: fuzz/Fuzz.cpp#L35-L42 (chrome/m156)
    #[doc(alias = "nextRange")]
    #[must_use]
    pub fn next_range_f32(&mut self, min: f32, max: f32) -> f32 {
        let mut f: f32 = self.next();
        if !f.is_normal() && f != 0.0 {
            // Don't deal with infinity or other strange floats.
            f = max;
        }
        // `std::fmod(float, float)` is exact, as is Rust's `%` on `f32`.
        min + f.abs() % (max - min + 1.0)
    }

    /// The next value of an enum's underlying integer type `U`, clamped to `[0, max]`
    /// (`nextEnum`). The caller converts it to the enum.
    // Port of: fuzz/Fuzz.h#L133-L144 (chrome/m156)
    #[doc(alias = "nextEnum")]
    #[must_use]
    pub fn next_enum<U: FuzzValue>(&mut self, max: U) -> U {
        let v: U = self.next();
        if v < U::zero() {
            return U::zero();
        }
        if v > max {
            return max;
        }
        v
    }

    /// The next `n` values of `T` (`nextN`).
    // Port of: fuzz/Fuzz.h#L146-L151 (chrome/m156)
    #[doc(alias = "nextN")]
    #[must_use]
    pub fn next_n<T: FuzzValue>(&mut self, n: usize) -> Vec<T> {
        (0..n).map(|_| self.next()).collect()
    }

    /// Ops up to 10 fuzzed rects into `region` (`next(SkRegion*)`, `FuzzNiceRegion(this, region,
    /// 10)`).
    // Port of: fuzz/Fuzz.cpp#L30-L33 (chrome/m156)
    pub fn next_region(&mut self, region: &mut Region) {
        crate::common::fuzz_nice_region(self, region, 10);
    }

    /// Tells the fuzzer that this input found a bug. Skia raises `SIGSEGV`; a panic is what
    /// libFuzzer and the replay tests report as a crash.
    ///
    /// # Panics
    /// Always.
    // Port of: fuzz/Fuzz.h#L84-L88 (chrome/m156)
    #[doc(alias = "signalBug")]
    pub fn signal_bug(&self) -> ! {
        panic!("Signal bug");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_reads_zero_fill_and_exhaust() {
        let mut fuzz = Fuzz::new(&[1, 2, 3]);
        assert_eq!(fuzz.next::<u16>(), u16::from_ne_bytes([1, 2]));
        assert_eq!(fuzz.next::<u32>(), u32::from_ne_bytes([3, 0, 0, 0]));
        assert!(fuzz.exhausted());
        assert_eq!(fuzz.next::<f64>(), 0.0);
        assert_eq!(fuzz.remaining_size(), 0);
    }

    #[test]
    fn bool_uses_the_low_bit() {
        let mut fuzz = Fuzz::new(&[2, 3]);
        assert!(!fuzz.next_bool());
        assert!(fuzz.next_bool());
    }

    #[test]
    fn range_clamps_and_keeps_nan() {
        let mut fuzz = Fuzz::new(&[200, 0x80]);
        assert_eq!(fuzz.next_range::<u8>(0, 9), 9);
        // (char)0x80 is -128 on x86, clamped up to 0x20.
        assert_eq!(fuzz.next_range::<i8>(0x20, 0x7E), 0x20);
        let nan = f32::NAN.to_ne_bytes();
        let mut fuzz = Fuzz::new(&nan);
        assert!(fuzz.next_range::<f32>(-1000.0, 1000.0).is_nan());
    }

    #[test]
    fn float_range_uses_fmod() {
        let mut fuzz = Fuzz::new(&[]);
        // Exhausted: 0.0 is not normal but is zero, so it stays: 0 + fmod(0, 101).
        assert_eq!(fuzz.next_range_f32(0.0, 100.0), 0.0);
        let bytes: Vec<u8> = [250.5f32, f32::INFINITY, -3.0]
            .iter()
            .flat_map(|f| f.to_ne_bytes())
            .collect();
        let mut fuzz = Fuzz::new(&bytes);
        assert_eq!(fuzz.next_range_f32(0.0, 100.0), 250.5 % 101.0);
        // Infinity becomes max: 0 + fmod(100, 101).
        assert_eq!(fuzz.next_range_f32(0.0, 100.0), 100.0);
        assert_eq!(fuzz.next_range_f32(-720.0, 720.0), -717.0);
    }

    #[test]
    fn enum_clamps_negative_to_zero() {
        let bytes: Vec<u8> = [-5i32, 9, 2].iter().flat_map(|v| v.to_ne_bytes()).collect();
        let mut fuzz = Fuzz::new(&bytes);
        assert_eq!(fuzz.next_enum(5i32), 0);
        assert_eq!(fuzz.next_enum(5i32), 5);
        assert_eq!(fuzz.next_enum(5i32), 2);
    }
}

//! A 96-bit signed integer: `hi` holds the upper 64 bits (signed), `lo` the lower 32 bits.
//! Port of `modules/bentleyottmann/include/Int96.h` and `src/Int96.cpp` (chrome/m156).

// Port of: modules/bentleyottmann/include/Int96.h#L13-L21 (chrome/m156)

/// A 96-bit integer used where a product of a 64-bit and a 32-bit value must not overflow.
///
/// Ordering is `(hi, lo)` lexicographic, matching the C++ `std::tie(a.hi, a.lo)` comparison.
#[doc(alias = "bentleyottmann::Int96")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Int96 {
    /// The upper 64 bits, signed.
    pub hi: i64,
    /// The lower 32 bits, unsigned.
    pub lo: u32,
}

impl From<i32> for Int96 {
    /// `Int96::Make(int32_t)`: sign-extends `a` into the upper 64 bits.
    // Port of: modules/bentleyottmann/src/Int96.cpp#L13-L15 (chrome/m156)
    #[doc(alias = "Make")]
    fn from(a: i32) -> Self {
        Self {
            hi: if a >= 0 { 0 } else { -1 },
            // C++ `(uint32_t)a`: the two's complement bit pattern.
            #[allow(clippy::cast_sign_loss)] // mirrors the C++ unsigned cast of a signed value
            lo: a as u32,
        }
    }
}

impl From<i64> for Int96 {
    /// `Int96::Make(int64_t)`: the upper 64 bits are `a >> 32` and the lower 32 bits are `a`.
    // Port of: modules/bentleyottmann/src/Int96.cpp#L17-L19 (chrome/m156)
    #[doc(alias = "Make")]
    fn from(a: i64) -> Self {
        Self {
            hi: a >> 32,
            // C++ `(uint32_t)(a & 0xFFFFFFFF)`.
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // masked to 32 bits
            lo: (a & 0xFFFF_FFFF) as u32,
        }
    }
}

impl std::ops::Add for Int96 {
    type Output = Self;

    /// `operator+(Int96, Int96)`: the carry out of the low words is added into the high word.
    // Port of: modules/bentleyottmann/src/Int96.cpp#L30-L35 (chrome/m156)
    fn add(self, rhs: Self) -> Self {
        let lo = self.lo.wrapping_add(rhs.lo);
        let carry = i64::from(lo < self.lo);
        let hi = self.hi.wrapping_add(rhs.hi).wrapping_add(carry);
        Self { hi, lo }
    }
}

/// Multiply a 64-bit int and a 32-bit int producing a 96-bit int. This proceeds in two
/// multiplies:
///
/// 1. An unsigned multiply of the low 32 bits of `a` and the 32 bits of `b`. The unsigned multiply
///    of the low word requires a compensation if `b` is negative: the low bits of `a` are
///    subtracted from the upper 64 bits of the result.
/// 2. A signed multiply of the upper 32 bits of `a` and `b`.
// Port of: modules/bentleyottmann/src/Int96.cpp#L40-L66 (chrome/m156) (`multiply(int64_t, int32_t)`)
#[must_use]
#[allow(
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation
)] // C++ unsigned/signed casts of the 32-bit and 64-bit words; each is exact by construction
pub fn multiply(a: i64, b: i32) -> Int96 {
    // Multiply the low 32-bits generating a 64-bit lower part.
    let lo_a = (a & 0xFFFF_FFFF) as u64;
    let lo_b = u64::from(b as u32);
    let new_lo = lo_a.wrapping_mul(lo_b);

    // Multiply the upper bits 32-bits of a and b resulting in the hi 64-bits of the Int96.
    let mut new_hi = (a >> 32).wrapping_mul(i64::from(b));

    // Calculate the overflow into the upper 32-bits. `new_lo` is unsigned, so the shift zero
    // fills.
    let low_overflow = (new_lo >> 32) as i64;

    // Add overflow from the low multiply into hi.
    new_hi = new_hi.wrapping_add(low_overflow);

    // Compensate for the negative b in the calculation of new_lo by subtracting out 2^32 * a.
    if b < 0 {
        new_hi = new_hi.wrapping_sub(lo_a as i64);
    }

    Int96 {
        hi: new_hi,
        lo: new_lo as u32,
    }
}

/// `multiply(int32_t a, int64_t b)`: the C++ overload that forwards to [`multiply`] with the
/// arguments swapped.
// Port of: modules/bentleyottmann/src/Int96.cpp#L67-L69 (chrome/m156)
#[must_use]
pub fn multiply_i32_i64(a: i32, b: i64) -> Int96 {
    multiply(b, a)
}

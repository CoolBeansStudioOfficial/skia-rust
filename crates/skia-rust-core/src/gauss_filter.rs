// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkGaussFilter.h, src/core/SkGaussFilter.cpp

//! `SkGaussFilter`: gaussian filters for values of sigma < 2, good to 1 part in 1,000,000.
//! Produces values as defined in "Scale-Space for Discrete Signals" by Tony Lindeberg.

/// `SkGaussFilter::kGaussArrayMax`.
pub const GAUSS_ARRAY_MAX: usize = 6;

// The value when we can stop expanding the filter. The spec implies that 3% is acceptable, but
// we just use 1%.
// Port of: src/core/SkGaussFilter.cpp#L16 (chrome/m156)
const GOOD_ENOUGH: f64 = 1.0 / 100.0;

// Normalize the values of gauss to 1.0, and make sure they add to one.
// NB if n == 1, then this will force gauss[0] == 1.
// Port of: src/core/SkGaussFilter.cpp#L18-L40 (chrome/m156)
fn normalize(n: usize, gauss: &mut [f64; GAUSS_ARRAY_MAX]) {
    // Carefully add from smallest to largest to calculate the normalizing sum.
    let mut sum = 0.0;
    for i in (1..n).rev() {
        sum += 2.0 * gauss[i];
    }
    sum += gauss[0];

    // Normalize gauss.
    for g in &mut gauss[..n] {
        *g /= sum;
    }

    // The factors should sum to 1. Take any remaining slop, and add it to gauss[0]. Add the
    // values in such a way to maintain the most accuracy.
    sum = 0.0;
    for i in (1..n).rev() {
        sum += 2.0 * gauss[i];
    }

    gauss[0] = 1.0 - sum;
}

// BesselI_0 for 0 <= sigma < 2.
// NB the k = 0 factor is just sum = 1.0.
fn bessel_i_0(t: f64) -> f64 {
    let t_squared_over_4 = t * t / 4.0;
    let mut sum = 1.0;
    let mut factor = 1.0;
    let mut k: i32 = 1;
    // Use a variable number of loops. When sigma is small, this only requires 3-4 loops, but
    // when sigma is near 2, it could require 10 loops. The same holds for BesselI_1.
    while factor > 1.0 / 1_000_000.0 {
        factor *= t_squared_over_4 / f64::from(k * k);
        sum += factor;
        k += 1;
    }
    sum
}

// BesselI_1 for 0 <= sigma < 2.
fn bessel_i_1(t: f64) -> f64 {
    let t_squared_over_4 = t * t / 4.0;
    let mut sum = t / 2.0;
    let mut factor = sum;
    let mut k: i32 = 1;
    while factor > 1.0 / 1_000_000.0 {
        factor *= t_squared_over_4 / f64::from(k * (k + 1));
        sum += factor;
        k += 1;
    }
    sum
}

// Port of: src/core/SkGaussFilter.cpp#L42-L106 (chrome/m156)
fn calculate_bessel_factors(sigma: f64, gauss: &mut [f64; GAUSS_ARRAY_MAX]) -> usize {
    let var = sigma * sigma;

    // The following formula for calculating the Gaussian kernel is from
    // "Scale-Space for Discrete Signals" by Tony Lindeberg.
    // gauss(n; var) = besselI_n(var) / (e^var)
    // skia-rust: libm (`f64::exp`)
    let d = var.exp();
    let mut b = [0.0f64; GAUSS_ARRAY_MAX];
    b[0] = bessel_i_0(var);
    b[1] = bessel_i_1(var);
    gauss[0] = b[0] / d;
    gauss[1] = b[1] / d;

    // The code below is tricky, and written to mirror the recursive equations from the book.
    // The maximum spread for sigma == 2 is guass[4], but in order to know to stop guass[5]
    // is calculated. At this point n == 5 meaning that gauss[0..4] are the factors, but a 6th
    // element was used to calculate them.
    let mut n: usize = 1;
    // The recurrence relation below is from "Numerical Recipes" 3rd Edition.
    // Equation 6.5.16 p.282
    while gauss[n] > GOOD_ENOUGH {
        #[allow(clippy::cast_precision_loss)] // mirrors the int -> double conversion of `2*n`
        let two_n = (2 * n) as f64;
        b[n + 1] = -(two_n / var) * b[n] + b[n - 1];
        gauss[n + 1] = b[n + 1] / d;
        n += 1;
    }

    normalize(n, gauss);

    n
}

/// Gaussian filter factors for a sigma in `[0, 2)`.
// Port of: src/core/SkGaussFilter.h#L18-L38 (chrome/m156)
#[doc(alias = "SkGaussFilter")]
#[derive(Clone, Debug)]
pub struct GaussFilter {
    basis: [f64; GAUSS_ARRAY_MAX],
    n: usize,
}

impl GaussFilter {
    /// Computes the filter for `sigma`, which must be in `[0, 2)`.
    // Port of: src/core/SkGaussFilter.cpp#L108-L112 (chrome/m156)
    #[must_use]
    pub fn new(sigma: f64) -> GaussFilter {
        debug_assert!((0.0..2.0).contains(&sigma));
        let mut basis = [0.0; GAUSS_ARRAY_MAX];
        let n = calculate_bessel_factors(sigma, &mut basis);
        GaussFilter { basis, n }
    }

    /// The number of factors.
    #[must_use]
    pub fn size(&self) -> usize {
        self.n
    }

    /// The radius of the filter: `size() - 1`.
    ///
    /// # Panics
    /// Never: the size is at most [`GAUSS_ARRAY_MAX`].
    #[must_use]
    pub fn radius(&self) -> i32 {
        i32::try_from(self.n).expect("n <= 6") - 1
    }

    /// The width of the filter: `2 * radius() + 1`.
    #[must_use]
    pub fn width(&self) -> i32 {
        2 * self.radius() + 1
    }

    /// The factors (`begin()..end()`).
    pub fn iter(&self) -> core::slice::Iter<'_, f64> {
        self.basis[..self.n].iter()
    }
}

impl<'a> IntoIterator for &'a GaussFilter {
    type Item = &'a f64;
    type IntoIter = core::slice::Iter<'a, f64>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
